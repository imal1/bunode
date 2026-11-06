//! The runner on its own: whatever holds the other end of its lifeline, the
//! process group ends when that end goes away (ADR-0003).

mod support;

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use support::*;

#[test]
fn killing_the_lifeline_holder_ends_the_group() {
    require_tool("node");
    let scratch = tempfile::tempdir().unwrap();
    let pids = scratch.path().join("pids");
    let (ours, theirs) = UnixStream::pair().unwrap();

    let mut runner = Command::new(runner_path())
        .args(["--stop-timeout-ms", "2000", "--", "node", "tree.js", "2"])
        .arg(&pids)
        .current_dir(fixture("process-tree"))
        .stdin(Stdio::from(OwnedFd::from(theirs)))
        .stdout(Stdio::null())
        .spawn()
        .unwrap();

    // Another process holds Bunode's end of the lifeline; ours is dropped.
    let mut holder = Command::new("sleep")
        .arg("60")
        .stdin(Stdio::from(OwnedFd::from(ours)))
        .spawn()
        .unwrap();

    let tree = wait_for_pids(&pids, 3);
    assert!(tree.iter().all(|&p| is_alive(p)));

    holder.kill().unwrap();
    holder.wait().unwrap();

    wait_until_dead(&tree);
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if runner.try_wait().unwrap().is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "the runner did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn reports_a_command_that_cannot_start() {
    let (ours, theirs) = UnixStream::pair().unwrap();
    let output = Command::new(runner_path())
        .args(["--", "/nonexistent/bunode-test-command"])
        .stdin(Stdio::from(OwnedFd::from(theirs)))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(127));
    drop(ours);
}

#[test]
fn if_the_runner_itself_is_killed_the_supervisor_ends_the_group() {
    require_tool("node");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let id = h.service(
        "process-tree",
        &format!("node tree.js 2 {}", pids.display()),
    );
    h.supervisor.start(id).unwrap();
    h.wait(id, "Running", |s| {
        s.status == bunode_core::RunStatus::Running
    });
    let tree = wait_for_pids(&pids, 3);

    // The runner is the parent of the tree's first process.
    let runner = bunode_core::sys::process(tree[0]).unwrap().ppid;
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(runner),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();

    let ended = h.wait(id, "the Run to end", |s| !s.status.is_live());
    assert_eq!(ended.status, bunode_core::RunStatus::Crashed, "{ended:?}");
    wait_until_dead(&tree);
}
