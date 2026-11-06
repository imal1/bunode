//! Editing and deleting Services (requirements §3.1, §3.2-11).

mod support;

use bunode_core::{LogStream, RunStatus, ServiceEdit};
use support::*;

fn edit(command: &str, shell: bool) -> ServiceEdit {
    ServiceEdit {
        command: command.into(),
        folder: None,
        port: None,
        run_through_shell: shell,
    }
}

fn stdout_of(h: &Harness, id: u64) -> Vec<String> {
    h.supervisor
        .log_lines(id, 0, 100)
        .into_iter()
        .filter(|l| l.stream == LogStream::Stdout)
        .map(|l| l.text)
        .collect()
}

#[test]
fn without_shell_mode_shell_syntax_is_just_arguments() {
    let h = Harness::new();
    let id = h.service("node-http", r#"echo "a  b" && echo $HOME | tr a-z A-Z"#);
    h.supervisor.start(id).unwrap();
    h.wait(id, "the echo", |s| {
        s.run_number.is_some() && !s.status.is_live()
    });
    assert_eq!(stdout_of(&h, id), ["a  b && echo $HOME | tr a-z A-Z"]);
}

#[test]
fn shell_mode_runs_pipes_and_chains() {
    let h = Harness::new();
    let id = h.service("node-http", "true");
    h.supervisor
        .update_service(id, edit("echo one && echo two | tr a-z A-Z", true))
        .unwrap();
    h.supervisor.start(id).unwrap();
    h.wait(id, "the shell", |s| {
        s.run_number.is_some() && !s.status.is_live()
    });
    assert_eq!(stdout_of(&h, id), ["one", "TWO"]);
}

#[test]
fn changes_while_running_apply_to_the_next_run() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("node-http", "node server.js");
    h.supervisor.start(id).unwrap();
    let running = h.wait(id, "Running", |s| s.status == RunStatus::Running);
    assert!(!running.has_pending_config);

    h.supervisor
        .update_service(id, edit("node server.js --changed", false))
        .unwrap();
    let pending = h.state(id);
    assert!(pending.has_pending_config);
    assert_eq!(pending.pid, running.pid, "the live Run is untouched");

    h.supervisor.restart(id).unwrap();
    let next = h.wait(id, "Run #2", |s| {
        s.status == RunStatus::Running && s.run_number == Some(2)
    });
    assert!(!next.has_pending_config);
    let first = h.supervisor.log_lines(id, 0, 1);
    assert_eq!(first[0].text, "Run #2: node server.js --changed");
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
}

#[test]
fn deleting_a_running_service_stops_it_first() {
    require_tool("node");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let id = h.service(
        "process-tree",
        &format!("node tree.js 2 {}", pids.display()),
    );
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    let tree = wait_for_pids(&pids, 3);

    h.supervisor.delete_service(id).unwrap();
    // Until the Run has ended, the Service is still there, stopping.
    assert_eq!(h.state(id).status, RunStatus::Stopping);
    h.supervisor.start(id).unwrap();

    let deadline = std::time::Instant::now() + TIMEOUT;
    while h
        .supervisor
        .snapshot()
        .projects
        .iter()
        .any(|p| p.services.iter().any(|s| s.id == id))
    {
        assert!(std::time::Instant::now() < deadline, "never deleted");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    wait_until_dead(&tree);
    assert!(fixture("process-tree").join("tree.js").exists());
}
