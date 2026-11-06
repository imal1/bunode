//! The ports Runs listen on (requirements §3.5).

mod support;

use bunode_core::{LogStream, RunStatus};
use support::*;

#[test]
fn finds_every_port_of_a_run_and_forgets_them_when_it_ends() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("multi-port", "node server.js");
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);

    // The server prints the two ports it got.
    let printed = loop {
        let line = h
            .supervisor
            .log_lines(id, 0, 100)
            .into_iter()
            .find(|l| l.stream == LogStream::Stdout && l.text.starts_with("listening on "));
        if let Some(line) = line {
            break line.text;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let mut expected: Vec<u16> = printed
        .trim_start_matches("listening on ")
        .split(" and ")
        .map(|p| p.parse().unwrap())
        .collect();
    expected.sort_unstable();

    let found = h.wait(id, "both ports", |s| s.live_ports == expected);
    assert_eq!(found.live_ports.len(), 2);

    h.supervisor.stop(id);
    let stopped = h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    assert!(stopped.live_ports.is_empty());
}

#[test]
fn finds_the_port_a_child_process_listens_on() {
    require_tool("npm");
    let h = Harness::new();
    // npm starts the server through sh: the port belongs to a grandchild.
    let id = h.service("multi-port", "npm start");
    h.supervisor.start(id).unwrap();
    let found = h.wait(id, "the ports", |s| s.live_ports.len() == 2);
    assert_eq!(found.status, RunStatus::Running);
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
}
