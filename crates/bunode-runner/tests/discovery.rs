//! Node and Bun processes Bunode did not start (requirements §3.3).

mod support;

use std::process::Command;
use std::time::Instant;

use bunode_core::{DiscoveredProcess, RunStatus};
use support::*;

/// Starts `command` in `folder` detached from this test process, the way a
/// terminal would, and returns its PID.
fn start_outside(folder: &str, command: &str) -> i32 {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("{command} >/dev/null 2>&1 & echo $!"))
        .current_dir(fixture(folder))
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap()
}

fn wait_for_discovered(
    h: &Harness,
    what: &str,
    found: impl Fn(&DiscoveredProcess) -> bool,
) -> DiscoveredProcess {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        h.supervisor.rescan();
        if let Some(p) = h
            .supervisor
            .snapshot()
            .discovered
            .into_iter()
            .find(|p| found(p))
        {
            return p;
        }
        assert!(Instant::now() < deadline, "never discovered {what}");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[test]
fn finds_processes_started_elsewhere_but_not_its_own() {
    require_tool("node");
    let h = Harness::new();
    let own = h.service("node-http", "node server.js --own");
    h.supervisor.start(own).unwrap();
    let own_pid = h
        .wait(own, "Running", |s| s.status == RunStatus::Running)
        .pid
        .unwrap();

    let pid = start_outside("node-http", "node server.js --outside");
    let found = wait_for_discovered(&h, "the outside server", |p| p.pid == pid as u32);
    assert_eq!(found.command, "node server.js --outside");
    assert_eq!(
        found.cwd.as_deref(),
        Some(fixture("node-http").to_str().unwrap())
    );
    assert!(!found.exiting);

    let snapshot = h.supervisor.snapshot();
    assert!(
        snapshot
            .discovered
            .iter()
            .all(|p| !p.members.contains(&own_pid)),
        "Bunode's own Run is never discovered"
    );

    h.supervisor
        .stop_discovered(found.pid, found.start)
        .unwrap();
    assert!(
        h.supervisor
            .snapshot()
            .discovered
            .iter()
            .any(|p| p.pid == found.pid && p.exiting)
    );
    wait_until_dead(&[pid]);
    let deadline = Instant::now() + TIMEOUT;
    while h
        .supervisor
        .snapshot()
        .discovered
        .iter()
        .any(|p| p.pid == found.pid)
    {
        h.supervisor.rescan();
        assert!(
            Instant::now() < deadline,
            "the exited process is still listed"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // A PID that no longer names the same process is refused.
    assert!(
        h.supervisor
            .stop_discovered(found.pid, found.start)
            .is_err()
    );
    h.supervisor.stop(own);
}

#[test]
fn a_package_manager_and_what_it_started_are_one_item() {
    require_tool("npm");
    let h = Harness::new();
    let pids = h.pid_file("pids");
    let launcher = start_outside(
        "process-tree",
        &format!("npm run tree -- {}", pids.display()),
    );
    let tree = wait_for_pids(&pids, 3);

    let found = wait_for_discovered(&h, "the npm tree", |p| {
        p.members.contains(&(tree[2] as u32))
    });
    assert!(found.command.starts_with("npm"), "{}", found.command);
    for pid in &tree {
        assert!(found.members.contains(&(*pid as u32)), "{pid} in {found:?}");
    }
    let listed = h
        .supervisor
        .snapshot()
        .discovered
        .iter()
        .filter(|p| p.members.iter().any(|m| tree.contains(&(*m as i32))))
        .count();
    assert_eq!(listed, 1, "one item, not one per process");

    h.supervisor
        .force_kill_discovered(found.pid, found.start)
        .unwrap();
    wait_until_dead(&tree);
    let _ = launcher;
}
