//! Shared helpers for the integration tests, which start real node and bun
//! processes through the real `bunode-runner` (requirements §8.2).

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use bunode_core::{
    AddProjectOutcome, Listener, NewService, ServiceState, Supervisor, SupervisorOptions,
};
use nix::sys::signal::kill;
use nix::unistd::Pid;
use tempfile::TempDir;

pub const TIMEOUT: Duration = Duration::from_secs(20);

/// Counts how often the Supervisor reports a change.
#[derive(Default)]
pub struct Counter(pub AtomicU64);

impl Listener for Counter {
    fn state_changed(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn runner_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_bunode-runner"))
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .canonicalize()
        .expect("fixture folder")
}

/// A Supervisor with its own data directory.
pub struct Harness {
    pub supervisor: Supervisor,
    pub changes: Arc<Counter>,
    pub scratch: TempDir,
    _data: TempDir,
}

static NAMES: AtomicU64 = AtomicU64::new(0);

impl Harness {
    pub fn new() -> Harness {
        let data = tempfile::tempdir().unwrap();
        let mut options = SupervisorOptions::new(data.path(), runner_path());
        // Tests run with the environment they were started with.
        options.login_shell = None;
        let changes = Arc::new(Counter::default());
        let supervisor = Supervisor::new(options, changes.clone());
        Harness {
            supervisor,
            changes,
            scratch: tempfile::tempdir().unwrap(),
            _data: data,
        }
    }

    /// Adds a Service running `command` in fixture folder `folder`.
    pub fn service(&self, folder: &str, command: &str) -> u64 {
        let project = match self.supervisor.add_project(&fixture(folder)).unwrap() {
            AddProjectOutcome::Added { project } | AddProjectOutcome::AlreadyAdded { project } => {
                project
            }
        };
        let name = format!("s{}", NAMES.fetch_add(1, Ordering::SeqCst));
        self.supervisor
            .add_service(
                project.id,
                NewService {
                    name,
                    command: command.into(),
                    folder: None,
                },
            )
            .unwrap()
            .id
    }

    pub fn state(&self, id: u64) -> ServiceState {
        self.supervisor
            .snapshot()
            .projects
            .into_iter()
            .flat_map(|p| p.services)
            .find(|s| s.id == id)
            .expect("service in snapshot")
    }

    /// Waits until the Service's state satisfies `done`.
    pub fn wait(&self, id: u64, what: &str, done: impl Fn(&ServiceState) -> bool) -> ServiceState {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let state = self.state(id);
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

    pub fn pid_file(&self, name: &str) -> PathBuf {
        self.scratch.path().join(name)
    }
}

/// Reads the PIDs a process-tree fixture wrote, waiting until there are `count`.
pub fn wait_for_pids(file: &Path, count: usize) -> Vec<i32> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let pids: Vec<i32> = fs::read_to_string(file)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect();
        if pids.len() >= count {
            return pids;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {count} pids in {}, got {pids:?}",
            file.display()
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// Whether a process exists and has not exited. Zombies count as exited:
/// in some containers nothing reaps orphans.
pub fn is_alive(pid: i32) -> bool {
    kill(Pid::from_raw(pid), None).is_ok() && !is_zombie(pid)
}

#[cfg(target_os = "linux")]
fn is_zombie(pid: i32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            let after_name = stat.rsplit_once(')')?.1.to_owned();
            after_name.split_whitespace().next().map(|s| s == "Z")
        })
        .unwrap_or(true)
}

#[cfg(not(target_os = "linux"))]
fn is_zombie(_pid: i32) -> bool {
    false
}

pub fn wait_until_dead(pids: &[i32]) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let alive: Vec<i32> = pids.iter().copied().filter(|&p| is_alive(p)).collect();
        if alive.is_empty() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "processes still alive: {alive:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// Fails the test early with a clear message when a tool is missing.
pub fn require_tool(tool: &str) {
    let found = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(tool).is_file()))
        .unwrap_or(false);
    assert!(found, "{tool} must be on PATH for the integration tests");
}
