//! Stability (requirements §5.1, §8.3). The quick checks run with every
//! `cargo test`; the long soaks are `#[ignore]`d and run on demand:
//!
//! ```text
//! BUNODE_SOAK_SECS=1800 cargo test --release -p bunode-runner --test stability -- --ignored --nocapture
//! ```
//!
//! See docs/stability.md.

mod support;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use bunode_core::{RunStatus, sys};
use support::*;

/// Every process still in one of these process groups.
fn survivors(groups: &HashSet<i32>) -> Vec<i32> {
    sys::all_pids()
        .into_iter()
        .filter_map(sys::process)
        .filter(|p| groups.contains(&p.pgid))
        .map(|p| p.pid)
        .filter(|&p| is_alive(p))
        .collect()
}

fn wait_for_no_survivors(groups: &HashSet<i32>) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let left = survivors(groups);
        if left.is_empty() {
            return;
        }
        assert!(Instant::now() < deadline, "processes left behind: {left:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_hundred_starts_and_stops_leave_nothing_behind() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("process-tree", "node tree.js 1 /dev/null");
    let mut groups = HashSet::new();
    for round in 1..=100u64 {
        h.supervisor.start(id).unwrap();
        let running = h.wait(id, "Running", |s| s.status == RunStatus::Running);
        assert_eq!(
            running.run_number,
            Some(round),
            "exactly one new Run per Start"
        );
        groups.insert(running.pid.unwrap() as i32);
        h.supervisor.stop(id);
        let stopped = h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
        assert_eq!(stopped.pid, None);
    }
    assert_eq!(groups.len(), 100);
    wait_for_no_survivors(&groups);
}

/// Start → Running latency of `count` Services started at once.
fn start_many(count: usize) -> Vec<Duration> {
    require_tool("node");
    let h = Harness::new();
    let ids: Vec<u64> = (0..count)
        .map(|i| h.service("node-http", &format!("node server.js --instance {i}")))
        .collect();
    let started = Instant::now();
    for &id in &ids {
        h.supervisor.start(id).unwrap();
    }
    let mut latencies = vec![None; count];
    let deadline = started + TIMEOUT;
    while latencies.iter().any(Option::is_none) {
        let snapshot = h.supervisor.snapshot();
        for (slot, id) in latencies.iter_mut().zip(&ids) {
            let running = snapshot
                .projects
                .iter()
                .flat_map(|p| &p.services)
                .any(|s| s.id == *id && s.status == RunStatus::Running);
            if running && slot.is_none() {
                *slot = Some(started.elapsed());
            }
        }
        assert!(Instant::now() < deadline, "not every Service came up");
        std::thread::sleep(Duration::from_millis(10));
    }
    let groups: HashSet<i32> = ids
        .iter()
        .map(|&id| h.state(id).pid.unwrap() as i32)
        .collect();
    for &id in &ids {
        h.supervisor.stop(id);
    }
    for &id in &ids {
        h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    }
    wait_for_no_survivors(&groups);
    // Everything the Supervisor reports matches: all stopped, none live.
    assert!(
        h.supervisor
            .snapshot()
            .projects
            .iter()
            .flat_map(|p| &p.services)
            .all(|s| s.status == RunStatus::Stopped && s.pid.is_none())
    );
    let mut latencies: Vec<Duration> = latencies.into_iter().map(Option::unwrap).collect();
    latencies.sort();
    latencies
}

fn report(label: &str, latencies: &[Duration]) {
    let at = |q: f64| latencies[((latencies.len() - 1) as f64 * q) as usize];
    eprintln!(
        "{label}: Start → Running median {:?}, 90th percentile {:?}, slowest {:?}",
        at(0.5),
        at(0.9),
        latencies.last().unwrap()
    );
}

#[test]
fn ten_services_at_once() {
    let latencies = start_many(10);
    report("10 Services", &latencies);
    // "Usually within 1 s" (requirements §5.2); CI machines get some slack.
    assert!(latencies[latencies.len() / 2] < Duration::from_secs(2));
}

#[test]
fn thirty_services_at_once() {
    let latencies = start_many(30);
    report("30 Services", &latencies);
    assert!(latencies[latencies.len() / 2] < Duration::from_secs(3));
}

fn soak_duration(default_secs: u64) -> Duration {
    Duration::from_secs(
        std::env::var("BUNODE_SOAK_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default_secs),
    )
}

/// Requirements §8.3: high-frequency logs for 30 minutes. Memory stays
/// bounded and change reports stay at no more than ten a second.
#[test]
#[ignore = "long: run with --ignored, BUNODE_SOAK_SECS sets the length (default 1800)"]
fn soak_a_flood_of_logs() {
    require_tool("node");
    let duration = soak_duration(1800);
    let h = Harness::new();
    let id = h.service("flood", "node flood.js");
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);

    let me = std::process::id() as i32;
    let started = Instant::now();
    // Let the log file get going before taking the baseline.
    std::thread::sleep(Duration::from_secs(10).min(duration / 4));
    let baseline = sys::resident_memory(me).unwrap();
    let reports_before = h.changes.0.load(Ordering::SeqCst);
    let measured_from = Instant::now();
    let mut peak = baseline;
    while started.elapsed() < duration {
        std::thread::sleep(Duration::from_secs(10).min(duration / 4));
        let memory = sys::resident_memory(me).unwrap();
        peak = peak.max(memory);
        let state = h.state(id);
        assert_eq!(
            state.status,
            RunStatus::Running,
            "the flood must not stop: {state:?}"
        );
        let file = h.supervisor.current_log_file(id);
        let on_disk = file
            .and_then(|f| std::fs::metadata(f).ok())
            .map_or(0, |m| m.len());
        eprintln!(
            "{:>5}s  memory {:>6} MB  current log file {:>6} MB",
            started.elapsed().as_secs(),
            memory / 1_048_576,
            on_disk / 1_048_576
        );
    }
    let elapsed = measured_from.elapsed().as_secs_f64();
    let reports = (h.changes.0.load(Ordering::SeqCst) - reports_before) as f64;
    eprintln!(
        "soak: {:.1} reports/s, memory {} MB at start, {} MB at peak",
        reports / elapsed,
        baseline / 1_048_576,
        peak / 1_048_576
    );
    assert!(reports / elapsed <= 11.0, "reports are coalesced");
    assert!(
        peak < baseline + 64 * 1_048_576,
        "memory grew from {baseline} to {peak} bytes"
    );
    let group = h.state(id).pid.unwrap() as i32;
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_for_no_survivors(&HashSet::from([group]));
}

/// Requirements §8.3: run for 24 hours, starting, stopping and restarting
/// ten Services over and over, checking after every round that each has
/// at most one Run, nothing is left behind and memory stays bounded.
#[test]
#[ignore = "long: run with --ignored, BUNODE_SOAK_SECS sets the length (default 86400)"]
fn soak_runs_for_a_day() {
    require_tool("node");
    let duration = soak_duration(86_400);
    let h = Harness::new();
    let ids: Vec<u64> = (0..10)
        .map(|i| h.service("node-http", &format!("node server.js --soak {i}")))
        .collect();
    let me = std::process::id() as i32;
    let baseline = sys::resident_memory(me).unwrap();
    let started = Instant::now();
    let mut round = 0u64;
    let mut groups = HashSet::new();
    while started.elapsed() < duration {
        round += 1;
        for (i, &id) in ids.iter().enumerate() {
            match (round as usize + i) % 3 {
                0 => {
                    h.supervisor.start(id).unwrap();
                }
                1 => h.supervisor.restart(id).unwrap(),
                _ => h.supervisor.stop(id),
            }
        }
        for &id in &ids {
            let state = h.wait(id, "a settled state", |s| {
                !s.status.is_live() || s.status == RunStatus::Running
            });
            if let Some(pid) = state.pid {
                groups.insert(pid as i32);
            }
            assert!(
                matches!(state.status, RunStatus::Running | RunStatus::Stopped),
                "{state:?}"
            );
        }
        // Only the groups of Runs that are running now may have processes.
        let live: HashSet<i32> = ids
            .iter()
            .filter_map(|&id| h.state(id).pid)
            .map(|p| p as i32)
            .collect();
        let ended: HashSet<i32> = groups.difference(&live).copied().collect();
        wait_for_no_survivors(&ended);
        if round % 50 == 0 {
            let memory = sys::resident_memory(me).unwrap();
            eprintln!(
                "{:>6}s  round {round}  memory {} MB",
                started.elapsed().as_secs(),
                memory / 1_048_576
            );
            assert!(
                memory < baseline + 64 * 1_048_576,
                "memory grew to {memory} bytes"
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    for &id in &ids {
        h.supervisor.stop(id);
    }
    for &id in &ids {
        h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    }
    wait_for_no_survivors(&groups);
}
