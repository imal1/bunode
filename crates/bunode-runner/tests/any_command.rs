//! A Service can run any executable, not only Node.js or Bun (ADR-0005).

mod support;

use bunode_core::RunStatus;
use support::*;

#[test]
fn runs_a_plain_executable_with_a_quoted_path_like_any_other_service() {
    require_tool("python3");
    let h = Harness::new();
    // The folder has no package.json; the path has a space in it.
    let tool = fixture("plain-tool").join("my tools/serve");
    let id = h.service("plain-tool", &format!("'{}' --no-sandbox", tool.display()));
    h.supervisor.start(id).unwrap();
    let running = h.wait(id, "Running", |s| s.status == RunStatus::Running);
    let pid = running.pid.unwrap() as i32;

    let printed = |prefix: &str| h.stdout(id).into_iter().find(|l| l.starts_with(prefix));
    h.wait(id, "the first log line", |_| {
        printed("plain tool started").is_some()
    });
    assert_eq!(
        printed("plain tool started").unwrap(),
        "plain tool started --no-sandbox"
    );
    h.wait(id, "the server's line", |_| {
        printed("Serving HTTP").is_some()
    });
    // "Serving HTTP on 127.0.0.1 port 54321 (http://127.0.0.1:54321/) ..."
    let port: u16 = printed("Serving HTTP")
        .unwrap()
        .split(" port ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|p| p.parse().ok())
        .expect("a port in the server's line");
    h.wait(id, "its port", |s| s.live_ports == vec![port]);

    h.supervisor.stop(id);
    let stopped = h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    assert!(stopped.live_ports.is_empty());
    wait_until_dead(&[pid]);
}
