//! Starting and stopping real Runs through the Supervisor and the runner.

mod support;

use std::fs;

use bunode_core::RunStatus;
use support::*;

fn is_running(s: &bunode_core::ServiceState) -> bool {
    s.status == RunStatus::Running && s.pid.is_some()
}

#[test]
fn node_http_server_starts_and_stops() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("node-http", "node server.js");

    h.supervisor.start(id).unwrap();
    let running = h.wait(id, "Running", is_running);
    let pid = running.pid.unwrap() as i32;
    assert_eq!(running.run_number, Some(1));
    assert!(is_alive(pid));

    h.supervisor.stop(id);
    let stopped = h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    assert_eq!(stopped.last_exit, None);
    wait_until_dead(&[pid]);
}

#[test]
fn bun_http_server_starts_and_stops() {
    require_tool("bun");
    let h = Harness::new();
    let id = h.service("bun-http", "bun server.ts");

    h.supervisor.start(id).unwrap();
    let pid = h.wait(id, "Running", is_running).pid.unwrap() as i32;
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&[pid]);
}

#[test]
fn stop_ends_every_process_of_the_group() {
    require_tool("node");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let id = h.service(
        "process-tree",
        &format!("node tree.js 2 {}", pids.display()),
    );

    h.supervisor.start(id).unwrap();
    let leader = h.wait(id, "Running", is_running).pid.unwrap() as i32;
    let tree = wait_for_pids(&pids, 3);
    assert_eq!(tree[0], leader);

    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&tree);
}

/// Package managers start the script through a shell and their own
/// processes; Stop must still end all of them.
#[test]
fn stop_ends_processes_started_by_package_managers() {
    let launchers = [
        ("npm", "npm run tree --"),
        ("pnpm", "pnpm tree"),
        ("bun", "bun run tree"),
    ];
    for (tool, launcher) in launchers {
        require_tool(tool);
        let h = Harness::new();
        let pids = h.pid_file("pids");
        let id = h.service("process-tree", &format!("{launcher} {}", pids.display()));

        h.supervisor.start(id).unwrap();
        let leader = h.wait(id, "Running", is_running).pid.unwrap() as i32;
        let tree = wait_for_pids(&pids, 3);

        h.supervisor.stop(id);
        h.wait(id, &format!("{tool}: Stopped"), |s| {
            s.status == RunStatus::Stopped
        });
        let mut all = tree.clone();
        all.push(leader);
        wait_until_dead(&all);
    }
}

#[test]
fn start_while_running_does_not_start_a_second_run() {
    require_tool("node");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let id = h.service(
        "process-tree",
        &format!("node tree.js 0 {}", pids.display()),
    );

    h.supervisor.start(id).unwrap();
    h.supervisor.start(id).unwrap();
    let running = h.wait(id, "Running", is_running);
    h.supervisor.start(id).unwrap();
    assert_eq!(h.state(id).run_number, Some(1));
    assert_eq!(wait_for_pids(&pids, 1).len(), 1);

    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&[running.pid.unwrap() as i32]);
    // Only one process was ever started.
    assert_eq!(fs::read_to_string(&pids).unwrap().lines().count(), 1);
}

#[test]
fn run_numbers_increase_and_persist() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("node-http", "node server.js");
    for expected in 1..=2 {
        h.supervisor.start(id).unwrap();
        let running = h.wait(id, "Running", is_running);
        assert_eq!(running.run_number, Some(expected));
        h.supervisor.stop(id);
        h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    }
}
