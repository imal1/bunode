//! Services that start with Bunode: each one gets a Run once per launch,
//! after the leftovers are gone and the login shell has answered
//! (requirements §3.8).

mod support;

use std::fs;
use std::net::TcpListener;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use bunode_core::{
    AddProjectOutcome, NewService, RunEnd, RunNote, RunStatus, ServiceEdit, ServiceState,
    Supervisor, SupervisorOptions, sys,
};
use support::*;
use tempfile::TempDir;

/// One launch of Bunode on the data directory `data`.
fn launch(data: &Path, configure: impl FnOnce(&mut SupervisorOptions)) -> Supervisor {
    let mut options = SupervisorOptions::new(data, runner_path());
    options.login_shell = None;
    configure(&mut options);
    Supervisor::new(options, Arc::new(Counter::default()))
}

fn relaunch(data: &Path) -> Supervisor {
    launch(data, |_| {})
}

/// Adds a Service running `command` in `folder`; `start` is its Start
/// with Bunode.
fn service(supervisor: &Supervisor, folder: &Path, command: &str, start: bool) -> u64 {
    let project = match supervisor.add_project(folder).unwrap() {
        AddProjectOutcome::Added { project } | AddProjectOutcome::AlreadyAdded { project } => {
            project
        }
    };
    let id = supervisor
        .add_service(
            project.id,
            NewService {
                name: format!("s{}", project.services.len()),
                command: command.into(),
                folder: None,
            },
        )
        .unwrap()
        .id;
    supervisor.set_start_with_bunode(id, start).unwrap();
    id
}

fn set_port(supervisor: &Supervisor, id: u64, port: u16) {
    supervisor
        .update_service(
            id,
            ServiceEdit {
                command: state(supervisor, id).command,
                folder: None,
                port: Some(port),
                run_through_shell: false,
            },
        )
        .unwrap();
}

fn state(supervisor: &Supervisor, id: u64) -> ServiceState {
    supervisor
        .snapshot()
        .projects
        .into_iter()
        .flat_map(|p| p.services)
        .find(|s| s.id == id)
        .expect("service in snapshot")
}

fn wait(
    supervisor: &Supervisor,
    id: u64,
    what: &str,
    done: impl Fn(&ServiceState) -> bool,
) -> ServiceState {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let state = state(supervisor, id);
        if done(&state) {
            return state;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}; last state: {state:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// Long enough for a Run that was going to start to have started.
fn settle() {
    thread::sleep(Duration::from_millis(700));
}

#[test]
fn a_new_service_does_not_start_with_bunode() {
    let data = TempDir::new().unwrap();
    let supervisor = relaunch(data.path());
    let id = service(&supervisor, &fixture("node-http"), "node server.js", false);
    assert!(!state(&supervisor, id).start_with_bunode);
    drop(supervisor);

    let supervisor = relaunch(data.path());
    settle();
    let after = state(&supervisor, id);
    assert_eq!(after.status, RunStatus::Stopped);
    assert_eq!(after.run_number, None);
}

#[test]
fn starts_the_chosen_services_once_at_each_launch() {
    require_tool("node");
    let data = TempDir::new().unwrap();
    let supervisor = relaunch(data.path());
    let chosen = service(&supervisor, &fixture("node-http"), "node server.js", true);
    let other = service(&supervisor, &fixture("bun-http"), "node -e 0", false);
    // Choosing it starts nothing while Bunode is running.
    settle();
    assert_eq!(state(&supervisor, chosen).status, RunStatus::Stopped);
    drop(supervisor);

    let supervisor = relaunch(data.path());
    let running = wait(&supervisor, chosen, "Running", |s| {
        s.status == RunStatus::Running
    });
    assert_eq!(running.run_number, Some(1));
    assert!(running.start_with_bunode, "kept across the launch");
    assert_eq!(state(&supervisor, other).run_number, None);

    // Stopped by hand, it stays stopped for the rest of this launch.
    supervisor.stop(chosen);
    wait(&supervisor, chosen, "Stopped", |s| {
        s.status == RunStatus::Stopped
    });
    settle();
    let stopped = state(&supervisor, chosen);
    assert_eq!(stopped.status, RunStatus::Stopped);
    assert_eq!(stopped.run_number, Some(1));
    assert_eq!(stopped.ended_by, Some(RunEnd::StoppedByUser));
    drop(supervisor);

    // The next launch goes by the saved choice, not by how the last ended.
    let supervisor = relaunch(data.path());
    let again = wait(&supervisor, chosen, "Running again", |s| {
        s.status == RunStatus::Running
    });
    assert_eq!(again.run_number, Some(2));
    assert_eq!(state(&supervisor, other).run_number, None);
}

#[test]
fn changing_the_choice_leaves_the_run_alone() {
    require_tool("node");
    let data = TempDir::new().unwrap();
    let supervisor = relaunch(data.path());
    let id = service(&supervisor, &fixture("node-http"), "node server.js", false);
    supervisor.start(id).unwrap();
    let running = wait(&supervisor, id, "Running", |s| {
        s.status == RunStatus::Running
    });

    for choice in [true, false] {
        supervisor.set_start_with_bunode(id, choice).unwrap();
        settle();
        let after = state(&supervisor, id);
        assert_eq!(after.start_with_bunode, choice);
        assert_eq!(after.status, RunStatus::Running);
        assert_eq!(after.pid, running.pid, "the same Run");
        assert_eq!(after.run_number, running.run_number);
        assert!(!after.has_pending_config, "nothing for a Run to apply");
    }
    drop(supervisor);

    let supervisor = relaunch(data.path());
    settle();
    assert_eq!(state(&supervisor, id).run_number, None, "switched off");
}

#[test]
fn every_failure_stays_on_its_own_service() {
    require_tool("node");
    let data = TempDir::new().unwrap();
    let gone = TempDir::new().unwrap();
    let taken_a = TcpListener::bind("127.0.0.1:0").unwrap();
    let taken_b = TcpListener::bind("127.0.0.1:0").unwrap();
    let port_a = taken_a.local_addr().unwrap().port();
    let port_b = taken_b.local_addr().unwrap().port();

    let supervisor = relaunch(data.path());
    let missing_command = service(
        &supervisor,
        &fixture("crash"),
        "bunode-no-such-command",
        true,
    );
    let missing_folder = service(&supervisor, gone.path(), "node -e 0", true);
    let conflict_a = service(
        &supervisor,
        &fixture("node-http"),
        "node server.js --a",
        true,
    );
    let conflict_b = service(
        &supervisor,
        &fixture("node-http"),
        "node server.js --b",
        true,
    );
    let healthy = service(&supervisor, &fixture("node-http"), "node server.js", true);
    set_port(&supervisor, conflict_a, port_a);
    set_port(&supervisor, conflict_b, port_b);
    drop(supervisor);
    let gone_path = gone.path().display().to_string();
    drop(gone);

    let supervisor = relaunch(data.path());
    // None of the four keeps the fifth from starting.
    wait(&supervisor, healthy, "Running", |s| {
        s.status == RunStatus::Running
    });
    let failed = wait(&supervisor, missing_command, "Failed", |s| {
        s.status == RunStatus::Failed
    });
    assert_eq!(
        failed.last_exit,
        Some(RunNote::NotInPath {
            program: "bunode-no-such-command".into()
        })
    );
    let failed = wait(&supervisor, missing_folder, "Failed", |s| {
        s.status == RunStatus::Failed
    });
    assert_eq!(
        failed.last_exit,
        Some(RunNote::FolderMissing { path: gone_path })
    );
    for (id, port) in [(conflict_a, port_a), (conflict_b, port_b)] {
        let blocked = wait(&supervisor, id, "its port conflict", |s| {
            s.startup_conflict.is_some()
        });
        let conflict = blocked.startup_conflict.unwrap();
        assert_eq!(conflict.port, port);
        assert!(conflict.suggestion.is_some_and(|s| s != port));
        assert_eq!(blocked.status, RunStatus::Stopped, "nothing started");
        assert_eq!(blocked.run_number, None);
    }

    // They are all still there after the others' news, and none is tried
    // again or moved to another port.
    supervisor.stop(healthy);
    wait(&supervisor, healthy, "Stopped", |s| {
        s.status == RunStatus::Stopped
    });
    settle();
    assert_eq!(state(&supervisor, missing_command).run_number, Some(1));
    assert_eq!(state(&supervisor, missing_folder).run_number, Some(1));
    assert!(state(&supervisor, conflict_a).startup_conflict.is_some());
    assert!(state(&supervisor, conflict_b).startup_conflict.is_some());

    // Dismissing one leaves the other; starting clears its own.
    supervisor.dismiss_startup_conflict(conflict_a);
    assert!(state(&supervisor, conflict_a).startup_conflict.is_none());
    assert!(state(&supervisor, conflict_b).startup_conflict.is_some());
    drop(taken_b);
    supervisor.start(conflict_b).unwrap();
    let started = wait(&supervisor, conflict_b, "a Run", |s| s.run_number.is_some());
    assert!(started.startup_conflict.is_none());
}

#[test]
fn a_short_lived_command_runs_once_per_launch() {
    require_tool("node");
    let data = TempDir::new().unwrap();
    let supervisor = relaunch(data.path());
    let id = service(
        &supervisor,
        &fixture("plain-tool"),
        "node -e \"console.log('built')\"",
        true,
    );
    drop(supervisor);

    for launch in 1..=2 {
        let supervisor = relaunch(data.path());
        let done = wait(&supervisor, id, "the Run to end", |s| {
            s.ended_by == Some(RunEnd::Exited)
        });
        assert_eq!(done.status, RunStatus::Stopped);
        assert_eq!(done.run_number, Some(launch));
        settle();
        assert_eq!(state(&supervisor, id).run_number, Some(launch), "once");
    }
}

/// A stand-in login shell: slow, and the only place `BUNODE_TEST_MARK`
/// comes from. Written by a child process, so that no fork of this one
/// holds it open for writing when it is executed.
fn slow_shell(dir: &Path) -> std::path::PathBuf {
    let shell = dir.join("shell");
    let script =
        "#!/bin/sh\nsleep 1\nBUNODE_TEST_MARK=late\nexport BUNODE_TEST_MARK\neval \"$4\"\n";
    let status = Command::new("sh")
        .arg("-c")
        .arg("printf '%s' \"$1\" > \"$2\" && chmod +x \"$2\"")
        .args(["sh", script])
        .arg(&shell)
        .status()
        .unwrap();
    assert!(status.success());
    shell
}

#[test]
fn waits_for_the_login_shell_environment() {
    require_tool("node");
    let data = TempDir::new().unwrap();
    let supervisor = relaunch(data.path());
    let id = service(
        &supervisor,
        &fixture("plain-tool"),
        "node -e \"console.log('mark=' + process.env.BUNODE_TEST_MARK)\"",
        true,
    );
    drop(supervisor);

    let shell = slow_shell(data.path());
    let supervisor = launch(data.path(), |options| options.login_shell = Some(shell));
    wait(&supervisor, id, "the Run to end", |s| {
        s.ended_by == Some(RunEnd::Exited)
    });
    assert!(supervisor.snapshot().environment.problem.is_none());
    let log = fs::read_to_string(supervisor.current_log_file(id).unwrap()).unwrap();
    assert!(log.contains("mark=late"), "{log}");
}

#[test]
fn starts_with_bunodes_own_environment_when_the_shell_fails() {
    require_tool("node");
    let data = TempDir::new().unwrap();
    let supervisor = relaunch(data.path());
    let id = service(&supervisor, &fixture("node-http"), "node server.js", true);
    drop(supervisor);

    let supervisor = launch(data.path(), |options| {
        options.login_shell = Some("/nonexistent/bunode-test-shell".into())
    });
    let running = wait(&supervisor, id, "Running", |s| {
        s.status == RunStatus::Running
    });
    assert_eq!(running.run_number, Some(1), "not tried again");
    assert!(
        supervisor.snapshot().environment.problem.is_some(),
        "the problem is kept"
    );
}

#[test]
fn ends_the_leftover_runs_first() {
    require_tool("node");
    let data = TempDir::new().unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let supervisor = relaunch(data.path());
    let id = service(&supervisor, &fixture("node-http"), "node server.js", true);
    set_port(&supervisor, id, port);
    drop(supervisor);

    // What a Bunode that crashed left behind: a server on the Service's
    // port, and its record.
    let mut leftover = Command::new("node")
        .arg("server.js")
        .current_dir(fixture("node-http"))
        .env("PORT", port.to_string())
        .process_group(0)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + TIMEOUT;
    while !bunode_core::discovery::port_in_use(port) {
        assert!(Instant::now() < deadline, "the leftover never listened");
        thread::sleep(Duration::from_millis(20));
    }
    let pid = leftover.id() as i32;
    let start = sys::process(pid).unwrap().start;
    fs::write(
        data.path().join("runs.json"),
        format!(
            r#"[{{"service":{id},"run":1,"runnerPid":-1,"runnerStart":0,"group":{pid},"groupStart":{start}}}]"#
        ),
    )
    .unwrap();

    let supervisor = relaunch(data.path());
    let started = wait(&supervisor, id, "a Run", |s| s.run_number.is_some());
    assert!(
        started.startup_conflict.is_none(),
        "the port was free by then"
    );
    assert!(leftover.wait().is_ok());
    assert!(!is_alive(pid));
}
