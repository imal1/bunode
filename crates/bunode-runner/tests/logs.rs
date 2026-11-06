//! Run output reaches the Run's bounded log (requirements §2.2, §3.4).

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
    let done = h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    assert_eq!(done.log_count, 4, "{done:?}");

    let lines = h.supervisor.log_lines(id, 0, 100);
    let summary: Vec<(LogStream, &str)> =
        lines.iter().map(|l| (l.stream, l.text.as_str())).collect();
    assert_eq!(summary[0].0, LogStream::System);
    assert!(summary[0].1.starts_with("Run #1: node -e"));
    assert!(summary.contains(&(LogStream::Stdout, "to stdout")));
    assert!(summary.contains(&(LogStream::Stderr, "to stderr")));
    assert_eq!(summary[3], (LogStream::System, "Exited"));
    let stderr = lines
        .iter()
        .find(|l| l.stream == LogStream::Stderr)
        .unwrap();
    assert_eq!(stderr.spans.len(), 1, "the color survives as a span");

    h.supervisor.clear_logs(id);
    assert_eq!(h.state(id).log_count, 0);
    assert!(h.supervisor.log_lines(id, 0, 100).is_empty());
}

#[test]
fn search_filters_and_follows_new_lines() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("flood", "node flood.js 1000");
    h.supervisor.set_log_filter(id, "LINE 99");
    h.supervisor.start(id).unwrap();
    let done = h.wait(id, "the flood to end", |s| {
        !s.status.is_live() && s.log_match_count == 11
    });
    let matches = h
        .supervisor
        .log_matching_lines(id, done.log_match_first, 100);
    let mut texts: Vec<&str> = matches.iter().map(|l| l.text.as_str()).collect();
    texts.sort();
    assert_eq!(texts[0], "line 99");
    assert!(texts.contains(&"line 999"));

    let generation = done.log_filter_generation;
    h.supervisor.set_log_filter(id, "");
    h.wait(id, "the search to go away", |s| {
        s.log_filter_generation != generation
    });
    assert_eq!(h.state(id).log_match_count, 0);
}

#[test]
fn export_saves_the_full_log_not_just_what_memory_keeps() {
    require_tool("node");
    let h = Harness::new();
    h.supervisor.set_log_lines_kept(5_000).unwrap();
    let id = h.service("flood", "node flood.js 20000");
    h.supervisor.start(id).unwrap();
    let done = h.wait(id, "the flood to end", |s| !s.status.is_live());
    assert_eq!(done.log_count, 5_000);

    h.supervisor.clear_logs(id);
    let out = h.scratch.path().join("flood-run1.log");
    h.supervisor.export_log(id, &out).unwrap();
    let exported = std::fs::read_to_string(&out).unwrap();
    let lines: Vec<&str> = exported.lines().collect();
    // Every line, Bunode's two included, even after Clear.
    assert_eq!(lines.len(), 20_002);
    assert!(lines[0].contains(" bunode Run #1: node flood.js 20000"));
    assert!(lines.iter().any(|l| l.ends_with(" err line 20000")));
    assert!(lines.iter().any(|l| l.ends_with(" out line 1")));
}

#[test]
fn a_flood_of_output_stays_bounded_and_coalesced() {
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
    // 200,000 lines plus the two system lines; the default keeps 10,000.
    assert_eq!(done.log_count, 10_000);
    assert_eq!(done.log_count + done.log_dropped, 200_002);
    // stdout and stderr are separate pipes read side by side, so either may
    // finish first; the one that finished last still has its final line.
    // Within each stream the order is kept, and Bunode's line comes last.
    let kept = h.supervisor.log_lines(id, 0, 10_000);
    assert!(
        kept.iter()
            .any(|l| l.text == "line 199999" || l.text == "line 200000")
    );
    for stream in [LogStream::Stdout, LogStream::Stderr] {
        let numbers: Vec<u64> = kept
            .iter()
            .filter(|l| l.stream == stream)
            .filter_map(|l| l.text.strip_prefix("line ")?.parse().ok())
            .collect();
        assert!(
            numbers.windows(2).all(|w| w[0] < w[1]),
            "{stream:?} out of order"
        );
    }
    assert_eq!(kept.last().unwrap().stream, LogStream::System);

    // At most one report per 100 ms, not one per line.
    let allowed = elapsed.as_millis() as u64 / 100 + 5;
    assert!(changes <= allowed, "{changes} reports in {elapsed:?}");
}
