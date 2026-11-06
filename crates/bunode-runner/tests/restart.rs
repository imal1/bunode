//! Restart ends the current Run completely before the next one starts
//! (requirements §3.2).

mod support;

use bunode_core::RunStatus;
use support::*;

#[test]
fn restart_ends_the_old_group_and_starts_a_new_run() {
    require_tool("node");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let id = h.service(
        "process-tree",
        &format!("node tree.js 1 {}", pids.display()),
    );

    h.supervisor.start(id).unwrap();
    h.wait(id, "Run #1", |s| {
        s.status == RunStatus::Running && s.run_number == Some(1)
    });
    let first = wait_for_pids(&pids, 2);

    h.supervisor.restart(id).unwrap();
    assert_eq!(h.state(id).status, RunStatus::Restarting);
    let second = h.wait(id, "Run #2", |s| {
        s.status == RunStatus::Running && s.run_number == Some(2) && s.pid.is_some()
    });
    // Everything of Run #1 is gone, its children included.
    wait_until_dead(&first);
    let all = wait_for_pids(&pids, 4);
    assert!(all[2..].iter().all(|&p| is_alive(p)));
    assert_eq!(second.pid.unwrap() as i32, all[2]);

    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&all);
}

#[test]
fn repeated_restarts_never_leave_two_runs() {
    require_tool("node");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let id = h.service(
        "process-tree",
        &format!("node tree.js 0 {}", pids.display()),
    );
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    wait_for_pids(&pids, 1);

    for _ in 0..10 {
        h.supervisor.restart(id).unwrap();
    }
    let state = h.wait(id, "the restart", |s| {
        s.status == RunStatus::Running && s.run_number == Some(2)
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(
        h.state(id).run_number,
        Some(2),
        "restarts while restarting are ignored"
    );

    let started = wait_for_pids(&pids, 2);
    let alive: Vec<i32> = started.iter().copied().filter(|&p| is_alive(p)).collect();
    assert_eq!(alive, [state.pid.unwrap() as i32]);

    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
}

#[test]
fn stop_cancels_a_pending_restart() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("node-http", "node server.js");
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    h.supervisor.restart(id).unwrap();
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let state = h.state(id);
    assert_eq!(
        (state.status, state.run_number),
        (RunStatus::Stopped, Some(1))
    );
}

#[test]
fn restarting_a_stopped_service_starts_it() {
    require_tool("node");
    let h = Harness::new();
    let id = h.service("node-http", "node server.js");
    h.supervisor.restart(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
}
