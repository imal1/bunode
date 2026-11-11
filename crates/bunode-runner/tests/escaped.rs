//! A command that starts its server in a new process group, as pnpm 12
//! does: the server is still part of the Run, for its ports and for every
//! way a Run ends (ADR-0010).

mod support;

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::Duration;

use bunode_core::RunStatus;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use support::*;

/// The launcher's and the server's PIDs, once the server listens. Checks
/// that the server really left the launcher's group.
fn launched(pids: &std::path::Path) -> [i32; 2] {
    let found = wait_for_pids(pids, 2);
    let (launcher, server) = (found[0], found[1]);
    let group = |pid| bunode_core::sys::process(pid).unwrap().pgid;
    assert_ne!(group(launcher), group(server), "the server left the group");
    [launcher, server]
}

/// Starts `node launcher.js` as a Service and waits for both processes.
fn start(h: &Harness, flags: &str) -> (u64, [i32; 2]) {
    require_tool("node");
    let pids = h.pid_file("pids");
    let id = h.service(
        "escaped-group",
        &format!("node launcher.js {} {flags}", pids.display()),
    );
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| s.status == RunStatus::Running);
    (id, launched(&pids))
}

#[test]
fn finds_the_port_of_a_server_outside_the_group() {
    let h = Harness::new();
    let (id, tree) = start(&h, "");
    let port: u16 = loop {
        let line = h
            .stdout(id)
            .into_iter()
            .find_map(|l| l.strip_prefix("listening on ").map(str::to_owned));
        if let Some(port) = line {
            break port.parse().unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    h.wait(id, "the server's port", |s| s.live_ports == [port]);
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&tree);
}

#[test]
fn stop_ends_a_server_outside_the_group() {
    let h = Harness::new();
    let (id, tree) = start(&h, "");
    h.supervisor.stop(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&tree);
}

#[test]
fn force_kill_ends_a_server_outside_the_group() {
    let h = Harness::new();
    let (id, tree) = start(&h, "--ignore-sigterm");
    h.supervisor.stop(id);
    std::thread::sleep(Duration::from_millis(500));
    h.supervisor.force_kill(id);
    h.wait(id, "Stopped", |s| s.status == RunStatus::Stopped);
    wait_until_dead(&tree);
}

#[test]
fn killing_the_runner_ends_a_server_outside_the_group() {
    let h = Harness::new();
    let (id, tree) = start(&h, "");
    let runner = bunode_core::sys::process(tree[0]).unwrap().ppid;
    kill(Pid::from_raw(runner), Signal::SIGKILL).unwrap();
    let ended = h.wait(id, "the Run to end", |s| !s.status.is_live());
    assert_eq!(ended.status, RunStatus::Crashed, "{ended:?}");
    wait_until_dead(&tree);
}

#[test]
fn losing_the_lifeline_ends_a_server_outside_the_group() {
    require_tool("node");
    let scratch = tempfile::tempdir().unwrap();
    let pids = scratch.path().join("pids");
    let (ours, theirs) = UnixStream::pair().unwrap();
    let mut runner = Command::new(runner_path())
        .args(["--stop-timeout-ms", "500", "--", "node", "launcher.js"])
        .arg(&pids)
        .arg("--ignore-sigterm")
        .current_dir(fixture("escaped-group"))
        .stdin(Stdio::from(OwnedFd::from(theirs)))
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let tree = launched(&pids);

    // Like Bunode crashing: the other end of the lifeline goes away.
    drop(ours);
    wait_until_dead(&tree);
    runner.wait().unwrap();
}
