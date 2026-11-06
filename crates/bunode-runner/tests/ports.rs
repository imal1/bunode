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

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn a_taken_port_is_reported_and_the_next_one_offered() {
    require_tool("node");
    let h = Harness::new();
    let port = free_port();
    let first = h.service("node-http", "node server.js --first");
    let second = h.service("node-http", "node server.js --second");
    for id in [first, second] {
        h.supervisor
            .update_service(
                id,
                bunode_core::ServiceEdit {
                    command: h.state(id).command,
                    folder: None,
                    port: Some(port),
                    run_through_shell: false,
                },
            )
            .unwrap();
    }

    // The fixture listens on $PORT, so the first one gets it the way
    // `Use :N` hands a port over.
    assert_eq!(
        h.supervisor.start_with_port(first, port).unwrap(),
        bunode_core::StartOutcome::Started
    );
    h.wait(first, "the first on the port", |s| {
        s.live_ports.contains(&port)
    });

    let outcome = h.supervisor.start(second).unwrap();
    let bunode_core::StartOutcome::PortInUse {
        port: taken,
        owner,
        suggestion,
    } = outcome
    else {
        panic!("expected a port conflict, got {outcome:?}");
    };
    assert_eq!(taken, port);
    let owner = owner.expect("the owner is known");
    assert_eq!(owner.service_id, Some(first), "{owner:?}");
    assert_eq!(
        h.state(second).status,
        RunStatus::Stopped,
        "nothing started"
    );
    let suggestion = suggestion.expect("a free port");
    assert!(suggestion > port);

    // Use :N runs this one Run with PORT=N; the Service keeps its Port.
    h.supervisor.start_with_port(second, suggestion).unwrap();
    let running = h.wait(second, "the second on the suggestion", |s| {
        s.live_ports.contains(&suggestion)
    });
    assert_eq!(running.port, Some(port));
    let lines = h.supervisor.log_lines(second, 0, 10);
    assert!(
        lines
            .iter()
            .any(|l| l.text == format!("PORT={suggestion} for this Run"))
    );

    // Bunode never stopped the first one.
    assert_eq!(h.state(first).status, RunStatus::Running);
    h.supervisor.stop(first);
    h.supervisor.stop(second);
    h.wait(first, "Stopped", |s| s.status == RunStatus::Stopped);
    h.wait(second, "Stopped", |s| s.status == RunStatus::Stopped);
}

#[test]
fn a_port_held_outside_bunode_names_its_process() {
    require_tool("node");
    let h = Harness::new();
    let port = free_port();
    let outside = std::process::Command::new("node")
        .args(["server.js"])
        .env("PORT", port.to_string())
        .current_dir(fixture("node-http"))
        .stdout(std::process::Stdio::null())
        .spawn();
    let mut outside = outside.unwrap();
    let deadline = std::time::Instant::now() + TIMEOUT;
    while !h.supervisor.port_in_use(port) {
        assert!(
            std::time::Instant::now() < deadline,
            "the outside server never listened"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let id = h.service("node-http", "node server.js");
    h.supervisor
        .update_service(
            id,
            bunode_core::ServiceEdit {
                command: "node server.js".into(),
                folder: None,
                port: Some(port),
                run_through_shell: false,
            },
        )
        .unwrap();
    let outcome = h.supervisor.start(id).unwrap();
    let bunode_core::StartOutcome::PortInUse { owner, .. } = outcome else {
        panic!("expected a port conflict, got {outcome:?}");
    };
    let owner = owner.unwrap();
    assert_eq!(owner.pid, outside.id());
    assert_eq!(owner.command, "node server.js");
    assert_eq!(owner.service_id, None);

    outside.kill().unwrap();
    outside.wait().unwrap();
    let deadline = std::time::Instant::now() + TIMEOUT;
    while h.supervisor.port_in_use(port) {
        assert!(
            std::time::Instant::now() < deadline,
            "the port was never freed"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        h.supervisor.start(id).unwrap(),
        bunode_core::StartOutcome::Started
    );
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    h.supervisor.stop(id);
}
