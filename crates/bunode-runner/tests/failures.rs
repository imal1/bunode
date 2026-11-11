//! Runs that fail to start, crash, or are hammered with clicks
//! (requirements §3.2, §5.1).

mod support;

use std::time::Duration;

use bunode_core::{RunNote, RunStatus};
use support::*;

#[test]
fn a_command_that_does_not_exist_fails() {
    let h = Harness::new();
    let id = h.service("crash", "bunode-no-such-command --flag");
    h.supervisor.start(id).unwrap();
    let failed = h.wait(id, "Failed", |s| s.status == RunStatus::Failed);
    assert_eq!(
        failed.last_exit,
        Some(RunNote::NotInPath {
            program: "bunode-no-such-command".into()
        })
    );
    assert_eq!(failed.pid, None);
}

#[test]
fn an_unparsable_command_fails() {
    let h = Harness::new();
    let id = h.service("crash", "node 'unclosed");
    h.supervisor.start(id).unwrap();
    let failed = h.wait(id, "Failed", |s| s.status == RunStatus::Failed);
    assert_eq!(failed.last_exit, Some(RunNote::UnmatchedQuote));
}

#[test]
fn exiting_with_an_error_at_once_is_a_crash() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("crash", "node immediately.js");
    h.supervisor.start(id).unwrap();
    let crashed = h.wait(id, "Crashed", |s| s.status == RunStatus::Crashed);
    assert_eq!(crashed.last_exit, Some(RunNote::ExitedWithCode { code: 1 }));
}

#[test]
fn crashing_while_running_is_a_crash() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("crash", "node later.js");
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    let crashed = h.wait(id, "Crashed", |s| s.status == RunStatus::Crashed);
    assert!(matches!(
        crashed.last_exit,
        Some(RunNote::ExitedWithCode { .. })
    ));
}

#[test]
fn a_crash_does_not_affect_other_runs() {
    require_tool("node");
    let h = Harness::new();
    let steady = h.service("node-http", "node server.js");
    let crashing = h.service("crash", "node later.js");
    h.supervisor.start(steady).unwrap();
    h.supervisor.start(crashing).unwrap();
    h.wait(crashing, "Crashed", |s| s.status == RunStatus::Crashed);
    let pid = h.state(steady).pid.expect("still running") as i32;
    assert_eq!(h.state(steady).status, RunStatus::Running);
    assert!(is_alive(pid));
    h.supervisor.stop(steady);
    h.wait(steady, "Stopped", |s| s.status == RunStatus::Stopped);
}

#[test]
fn force_kill_ends_a_run_that_ignores_sigterm() {
    require_tool("node");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let id = h.service(
        "process-tree",
        &format!(
            // The handler goes in before the pid is written: a SIGTERM in
            // between would otherwise end it and fail the test.
            "node -e \"process.on('SIGTERM', () => {{}}); require('fs').appendFileSync('{}', process.pid + '\\n'); setInterval(() => {{}}, 1000)\"",
            pids.display()
        ),
    );
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    let pid = wait_for_pids(&pids, 1)[0];

    h.supervisor.stop(id);
    std::thread::sleep(Duration::from_millis(500));
    assert!(is_alive(pid), "SIGTERM is ignored, so it is still alive");
    h.supervisor.force_kill(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&[pid]);
}

#[test]
fn rapid_start_and_stop_never_leaves_two_runs() {
    require_tool("node");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let id = h.service(
        "process-tree",
        &format!("node tree.js 0 {}", pids.display()),
    );

    for _ in 0..20 {
        h.supervisor.start(id).unwrap();
        h.supervisor.start(id).unwrap();
        h.supervisor.stop(id);
        h.supervisor.start(id).unwrap();
    }
    // Let everything settle, then make sure exactly one Run is left.
    let deadline = std::time::Instant::now() + TIMEOUT;
    loop {
        let state = h.state(id);
        if !state.status.is_live() {
            h.supervisor.start(id).unwrap();
        }
        if state.status == RunStatus::Running {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "never settled: {state:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(500));
    let started = wait_for_pids(&pids, 1);
    let alive: Vec<i32> = started.iter().copied().filter(|&p| is_alive(p)).collect();
    assert_eq!(
        alive.len(),
        1,
        "exactly one live process, got {alive:?} of {started:?}"
    );

    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&started);
}
