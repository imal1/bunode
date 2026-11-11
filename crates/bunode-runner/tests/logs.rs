//! Run output reaches the Run's log file, which Console shows
//! (requirements §3.4).

mod support;

use std::sync::atomic::Ordering;
use std::time::Instant;

use bunode_core::{LogStream, RunStatus};
use support::*;

#[test]
fn collects_stdout_stderr_and_system_lines() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service(
        "node-http",
        r#"node -e "console.log('to stdout'); console.error('\x1b[31mto stderr\x1b[0m')""#,
    );
    h.supervisor.start(id).unwrap();
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);

    let log = h.log(id);
    assert_eq!(log.len(), 4, "{log:?}");
    assert_eq!(log[0].0, LogStream::System);
    assert!(log[0].1.starts_with("Run #1: node -e"));
    assert!(log.contains(&(LogStream::Stdout, "to stdout".into())));
    // Colors don't reach the file.
    assert!(log.contains(&(LogStream::Stderr, "to stderr".into())));
    assert_eq!(log[3], (LogStream::System, "Exited".into()));
}

#[test]
fn the_current_log_file_is_the_runs_newest() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("flood", "node flood.js 10");
    assert_eq!(h.supervisor.current_log_file(id), None);
    h.supervisor.start(id).unwrap();
    h.wait(id, "the flood to end", |s| !s.status.is_live());
    let file = h.supervisor.current_log_file(id).unwrap();
    assert!(file.ends_with("run-1-000.log"), "{file:?}");
    assert!(h.stdout(id).contains(&"line 1".to_owned()));
}

#[test]
fn a_live_run_s_lines_are_on_disk_as_they_come() {
    require_tool("node");
    let h = Harness::new();
    // Prints once, then stays up without printing anything else.
    let id = h.service(
        "node-http",
        r#"node -e "console.log('up'); setInterval(() => {}, 1000)""#,
    );
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    let file = loop {
        if let Some(file) = h.supervisor.current_log_file(id) {
            break file;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    // Read the file directly, without the flush current_log_file does.
    let deadline = Instant::now() + TIMEOUT;
    while !std::fs::read_to_string(&file)
        .unwrap_or_default()
        .contains(" out up")
    {
        assert!(Instant::now() < deadline, "the line never reached the disk");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
}

#[test]
fn a_flood_of_output_reaches_the_file_in_order_and_coalesced() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("flood", "node flood.js 200000");
    let before = h.changes.0.load(Ordering::SeqCst);
    let started = Instant::now();

    h.supervisor.start(id).unwrap();
    let done = h.wait(id, "the flood to end", |s| !s.status.is_live());
    let elapsed = started.elapsed();
    let changes = h.changes.0.load(Ordering::SeqCst) - before;
    assert_eq!(done.status, RunStatus::Stopped, "{done:?}");

    let log = h.log(id);
    let skipped = log
        .iter()
        .any(|(_, text)| text.ends_with("because the disk could not keep up."));
    if !skipped {
        // 200,000 lines plus Bunode's two.
        assert_eq!(log.len(), 200_002);
    }
    // stdout and stderr are separate pipes read side by side, so they
    // interleave; within each stream the order is kept.
    for stream in [LogStream::Stdout, LogStream::Stderr] {
        let numbers: Vec<u64> = log
            .iter()
            .filter(|(s, _)| *s == stream)
            .filter_map(|(_, text)| text.strip_prefix("line ")?.parse().ok())
            .collect();
        assert!(
            numbers.windows(2).all(|w| w[0] < w[1]),
            "{stream:?} out of order"
        );
    }
    assert_eq!(log.last().unwrap().0, LogStream::System);

    // Lines don't change the snapshot: a handful of reports, not one per line.
    let allowed = elapsed.as_millis() as u64 / 100 + 5;
    assert!(changes <= allowed, "{changes} reports in {elapsed:?}");
}
