//! A Run's processes: its process group, and the descendants that left it
//! (ADR-0009).
//!
//! A Service's command leads a process group, and signalling the group
//! normally reaches everything. But a program may start a child in a new
//! group or session (pnpm 12 runs scripts that way), and that child and its
//! own children escape the group. They are still descendants: following
//! parents leads back into the group. So a Run is its group plus every
//! process whose parent is part of the Run.

use std::collections::{HashMap, HashSet};

use nix::errno::Errno;
use nix::sys::signal::{Signal, kill, killpg};
use nix::unistd::Pid;

use crate::sys::{self, ProcessInfo};

/// The PIDs of the Run whose process group is `group`, sorted: the group's
/// processes and all their descendants, in the group or not.
pub fn members(group: i32, infos: &HashMap<i32, ProcessInfo>) -> Vec<i32> {
    let mut children: HashMap<i32, Vec<i32>> = HashMap::new();
    for info in infos.values().filter(|p| p.pid > 1) {
        children.entry(info.ppid).or_default().push(info.pid);
    }
    let mut found: HashSet<i32> = infos
        .values()
        .filter(|p| p.pgid == group && p.pid > 1)
        .map(|p| p.pid)
        .collect();
    let mut queue: Vec<i32> = found.iter().copied().collect();
    while let Some(pid) = queue.pop() {
        for &child in children.get(&pid).into_iter().flatten() {
            if found.insert(child) {
                queue.push(child);
            }
        }
    }
    let mut found: Vec<i32> = found.into_iter().collect();
    found.sort_unstable();
    found
}

/// One Run's processes, for ending them. The members outside the group are
/// remembered by identity each time the Run is signalled: once their parent
/// is gone (SIGTERM ended it), nothing else leads to them.
#[derive(Debug, Clone)]
pub struct RunTree {
    group: i32,
    /// PID → start time of the members outside the group.
    escaped: HashMap<i32, u64>,
}

impl RunTree {
    pub fn new(group: i32) -> RunTree {
        RunTree {
            group,
            escaped: HashMap::new(),
        }
    }

    pub fn group(&self) -> i32 {
        self.group
    }

    /// Reads the process table, adding the members outside the group to
    /// those already known, and forgetting those that are gone.
    pub fn refresh(&mut self) {
        let infos = sys::processes();
        self.escaped
            .retain(|pid, start| infos.get(pid).is_some_and(|p| p.start == *start));
        for pid in members(self.group, &infos) {
            let info = &infos[&pid];
            if info.pgid != self.group {
                self.escaped.insert(pid, info.start);
            }
        }
    }

    /// Sends `signal` to the group and to every member outside it that is
    /// still the same process. One that leads a group of its own gets it
    /// as a group, which also reaches children started since.
    pub fn signal(&mut self, signal: Signal) {
        self.refresh();
        let _ = killpg(Pid::from_raw(self.group), signal);
        for (&pid, &start) in &self.escaped {
            let Some(info) = sys::process(pid).filter(|p| p.start == start) else {
                continue;
            };
            let target = Pid::from_raw(pid);
            let _ = if info.pgid == pid {
                killpg(target, signal)
            } else {
                kill(target, signal)
            };
        }
    }

    /// Whether nothing of the Run is left: the group is empty and every
    /// known member outside it has exited.
    pub fn is_gone(&self) -> bool {
        killpg(Pid::from_raw(self.group), None) == Err(Errno::ESRCH)
            && !self
                .escaped
                .iter()
                .any(|(&pid, &start)| sys::is_same_process(pid, start))
    }

    /// The members outside the group known so far.
    pub fn escaped(&self) -> Vec<i32> {
        let mut pids: Vec<i32> = self.escaped.keys().copied().collect();
        pids.sort_unstable();
        pids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: &[(i32, i32, i32)]) -> HashMap<i32, ProcessInfo> {
        rows.iter()
            .map(|&(pid, ppid, pgid)| {
                let info = ProcessInfo {
                    pid,
                    ppid,
                    pgid,
                    uid: 501,
                    start: pid as u64,
                    name: "node".into(),
                };
                (pid, info)
            })
            .collect()
    }

    #[test]
    fn follows_descendants_that_left_the_group() {
        // The pnpm 12 tree: runner 10 → pnpm 11 (group 11) → pnpm-native 12
        // → sh 13 (group 13) → node 14. 20 and 30 are strangers.
        let infos = table(&[
            (1, 0, 1),
            (9, 1, 9),
            (10, 9, 10),
            (11, 10, 11),
            (12, 11, 11),
            (13, 12, 13),
            (14, 13, 13),
            (20, 1, 20),
            (30, 9, 9),
        ]);
        assert_eq!(members(11, &infos), [11, 12, 13, 14]);
    }

    #[test]
    fn group_members_count_even_when_orphaned() {
        let infos = table(&[(1, 0, 1), (11, 1, 11), (12, 1, 11)]);
        assert_eq!(members(11, &infos), [11, 12]);
        assert!(members(99, &infos).is_empty());
    }

    #[test]
    fn ends_a_child_that_started_a_new_session() {
        use std::os::unix::process::CommandExt;
        use std::time::{Duration, Instant};
        // sh leads the group; its child setsid()s away from it.
        let mut leader = std::process::Command::new("sh")
            .args(["-c", "perl -e 'use POSIX; setsid(); sleep 60' & wait"])
            .process_group(0)
            .spawn()
            .unwrap();
        let group = leader.id() as i32;
        let mut tree = RunTree::new(group);
        let deadline = Instant::now() + Duration::from_secs(5);
        while tree.escaped().is_empty() {
            assert!(Instant::now() < deadline, "the child never left the group");
            std::thread::sleep(Duration::from_millis(20));
            tree.refresh();
        }
        let child = tree.escaped()[0];
        tree.signal(Signal::SIGKILL);
        leader.wait().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !tree.is_gone() {
            assert!(Instant::now() < deadline, "{child} survived");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
