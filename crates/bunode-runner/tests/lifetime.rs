//! Runs never outlive Bunode (ADR-0003): not when it is killed, and not
//! after a crash, when the next launch ends what survived.

mod support;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use bunode_core::leftovers::RunRecord;
use bunode_core::{
    AddProjectOutcome, Listener, NewService, RunEnd, RunStatus, Supervisor, SupervisorOptions,
};
use support::*;

struct Quiet;

impl Listener for Quiet {
    fn state_changed(&self) {}
}

/// Not a test of its own: the body of a stand-in Bunode that
/// `killing_the_app_ends_its_runs` starts and then kills. Without
/// `BUNODE_TEST_HOST` it does nothing.
#[test]
fn stand_in_app() {
    let Ok(spec) = std::env::var("BUNODE_TEST_HOST") else {
        return;
    };
    let mut parts = spec.splitn(3, '\n');
    let (data, folder, command) = (
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap(),
    );
    let mut options = SupervisorOptions::new(data, runner_path());
    options.login_shell = None;
    let supervisor = Supervisor::new(options, Arc::new(Quiet));
    let project = match supervisor.add_project(folder.as_ref()).unwrap() {
        AddProjectOutcome::Added { project } | AddProjectOutcome::AlreadyAdded { project } => {
            project
        }
    };
    let service = supervisor
        .add_service(
            project.id,
            NewService {
                name: "app".into(),
                command: command.into(),
                folder: None,
            },
        )
        .unwrap();
    supervisor.start(service.id).unwrap();
    loop {
        let state = supervisor
            .snapshot()
            .projects
            .into_iter()
            .flat_map(|p| p.services)
            .find(|s| s.id == service.id)
            .unwrap();
        if state.status == RunStatus::Running
            && let Some(pid) = state.pid
        {
            println!("stand-in running {pid}");
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_secs(120));
}

#[test]
fn killing_the_app_ends_its_runs() {
    require_tool("node");
    let data = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let pids = scratch.path().join("pids");
    let spec = format!(
        "{}\n{}\nnode tree.js 2 {}",
        data.path().display(),
        fixture("process-tree").display(),
        pids.display()
    );
    let mut app = Command::new(std::env::current_exe().unwrap())
        .args(["stand_in_app", "--exact", "--nocapture", "--test-threads=1"])
        .env("BUNODE_TEST_HOST", spec)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = BufReader::new(app.stdout.take().unwrap());
    for line in stdout.lines() {
        // libtest prints "test stand_in_app ... " first, on the same line.
        if line.unwrap().contains("stand-in running ") {
            break;
        }
    }
    let tree = wait_for_pids(&pids, 3);
    for &pid in &tree {
        if !is_alive(pid) {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            let _ = app.kill();
            panic!("{pid} is not alive: {stat}");
        }
    }

    // Like a crash or a force quit: no chance to clean up.
    app.kill().unwrap();
    app.wait().unwrap();
    wait_until_dead(&tree);
}

#[test]
fn the_next_launch_ends_leftovers_and_spares_reused_pids() {
    let data = tempfile::tempdir().unwrap();
    use std::os::unix::process::CommandExt;
    let mut leftover = Command::new("sleep")
        .arg("60")
        .process_group(0)
        .spawn()
        .unwrap();
    let mut stranger = Command::new("sleep")
        .arg("60")
        .process_group(0)
        .spawn()
        .unwrap();
    let start = |pid: u32| bunode_core::sys::process(pid as i32).unwrap().start;
    let records = vec![
        RunRecord {
            service: 1,
            run: 4,
            runner_pid: -1,
            runner_start: 0,
            group: Some(leftover.id() as i32),
            group_start: Some(start(leftover.id())),
        },
        RunRecord {
            service: 2,
            run: 1,
            runner_pid: -1,
            runner_start: 0,
            group: Some(stranger.id() as i32),
            group_start: Some(start(stranger.id()) + 7),
        },
    ];
    std::fs::write(data.path().join("runs.json"), serde_json_lite(&records)).unwrap();

    let mut options = SupervisorOptions::new(data.path(), runner_path());
    options.login_shell = None;
    let _supervisor = Supervisor::new(options, Arc::new(Quiet));

    let deadline = std::time::Instant::now() + TIMEOUT;
    while leftover.try_wait().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the leftover was not ended"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        stranger.try_wait().unwrap().is_none(),
        "a reused PID is never touched"
    );
    stranger.kill().unwrap();
    stranger.wait().unwrap();
    // The records are gone: the next launch has nothing to clean.
    assert_eq!(
        std::fs::read_to_string(data.path().join("runs.json")).unwrap(),
        "[]"
    );
}

/// `runs.json` as the Supervisor writes it.
fn serde_json_lite(records: &[RunRecord]) -> String {
    let items: Vec<String> = records
        .iter()
        .map(|r| {
            format!(
                r#"{{"service":{},"run":{},"runnerPid":{},"runnerStart":{},"group":{},"groupStart":{}}}"#,
                r.service,
                r.run,
                r.runner_pid,
                r.runner_start,
                r.group.unwrap(),
                r.group_start.unwrap()
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

#[test]
fn quitting_stops_every_run_and_says_why() {
    require_tool("node");
    let h = Harness::new();
    let a = h.service("node-http", "node server.js");
    let b = h.service("node-http", "node server.js --second");
    h.supervisor.start(a).unwrap();
    h.supervisor.start(b).unwrap();
    h.wait(a, "a running", |s| s.status == RunStatus::Running);
    h.wait(b, "b running", |s| s.status == RunStatus::Running);

    assert_eq!(h.supervisor.stop_all(), 2);
    for id in [a, b] {
        let ended = h.wait(id, "stopped", |s| s.status == RunStatus::Stopped);
        assert_eq!(ended.ended_by, Some(RunEnd::BunodeQuit));
        let last = h.supervisor.log_lines(id, 0, 100).pop().unwrap();
        assert_eq!(last.text, "Stopped because Bunode quit");
    }
    assert_eq!(h.supervisor.stop_all(), 0);

    h.supervisor.start(a).unwrap();
    h.wait(a, "a running again", |s| s.status == RunStatus::Running);
    h.supervisor.stop(a);
    let ended = h.wait(a, "stopped", |s| s.status == RunStatus::Stopped);
    assert_eq!(ended.ended_by, Some(RunEnd::StoppedByUser));
}
