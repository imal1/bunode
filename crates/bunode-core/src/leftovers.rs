//! Bunode remembers every live Run's processes on disk, so that after a
//! crash or a force quit the next launch can end whatever survived
//! (ADR-0003). A process is only signalled when both its PID and its start
//! time still match: a reused PID belongs to someone else and is never
//! touched.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, kill, killpg};
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};

use crate::model::ServiceId;
use crate::sys;

/// A live Run's processes: its runner, and the command's process group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub service: ServiceId,
    pub run: u64,
    pub runner_pid: i32,
    pub runner_start: u64,
    /// The command, which leads the Run's process group.
    pub group: Option<i32>,
    pub group_start: Option<u64>,
}

/// `runs.json` in the data directory.
pub struct RunRecords {
    path: PathBuf,
    records: Vec<RunRecord>,
}

impl RunRecords {
    pub fn new(data_dir: &Path) -> RunRecords {
        RunRecords {
            path: data_dir.join("runs.json"),
            records: Vec::new(),
        }
    }

    /// What the previous Bunode left behind.
    pub fn load(&self) -> Vec<RunRecord> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn add(&mut self, record: RunRecord) {
        self.records.retain(|r| r.service != record.service);
        self.records.push(record);
        self.save();
    }

    pub fn set_group(&mut self, service: ServiceId, run: u64, pid: i32) {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|r| r.service == service && r.run == run)
        else {
            return;
        };
        record.group = Some(pid);
        record.group_start = sys::process(pid).map(|p| p.start);
        self.save();
    }

    pub fn remove(&mut self, service: ServiceId, run: u64) {
        let before = self.records.len();
        self.records
            .retain(|r| !(r.service == service && r.run == run));
        if self.records.len() != before {
            self.save();
        }
    }

    /// Writes the current records, replacing what is on disk.
    pub fn save(&self) {
        let _ = (|| -> io::Result<()> {
            if let Some(dir) = self.path.parent() {
                fs::create_dir_all(dir)?;
            }
            let tmp = self.path.with_extension("json.tmp");
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&serde_json::to_vec(&self.records)?)?;
            file.sync_all()?;
            fs::rename(tmp, &self.path)
        })();
    }
}

/// Ends the processes of `records` whose identity still matches, SIGTERM
/// first, SIGKILL after `grace`. Returns a line for the diagnostics log for
/// each one, signalled or spared.
pub fn clean_up(records: &[RunRecord], grace: Duration) -> Vec<String> {
    let mut report = Vec::new();
    let mut signalled: Vec<(i32, u64, bool)> = Vec::new();
    for record in records {
        if let (Some(group), Some(start)) = (record.group, record.group_start) {
            if sys::is_same_process(group, start) {
                let _ = killpg(Pid::from_raw(group), Signal::SIGTERM);
                signalled.push((group, start, true));
                report.push(format!(
                    "Ended the leftover process group {group} of Service {} Run #{}.",
                    record.service, record.run
                ));
            } else if sys::process(group).is_some() {
                report.push(format!(
                    "Left PID {group} alone: it is not the process Bunode started (PID reused)."
                ));
            }
        }
        if sys::is_same_process(record.runner_pid, record.runner_start) {
            let _ = kill(Pid::from_raw(record.runner_pid), Signal::SIGTERM);
            signalled.push((record.runner_pid, record.runner_start, false));
        }
    }

    let deadline = Instant::now() + grace;
    while Instant::now() < deadline
        && signalled
            .iter()
            .any(|&(pid, start, _)| sys::is_same_process(pid, start))
    {
        thread::sleep(Duration::from_millis(25));
    }
    for (pid, start, is_group) in signalled {
        if sys::is_same_process(pid, start) {
            let target = Pid::from_raw(pid);
            let _ = if is_group {
                killpg(target, Signal::SIGKILL)
            } else {
                kill(target, Signal::SIGKILL)
            };
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;

    #[test]
    fn records_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut records = RunRecords::new(dir.path());
        records.add(RunRecord {
            service: 1,
            run: 3,
            runner_pid: 10,
            runner_start: 20,
            group: None,
            group_start: None,
        });
        assert_eq!(RunRecords::new(dir.path()).load().len(), 1);
        records.remove(1, 3);
        assert!(RunRecords::new(dir.path()).load().is_empty());
    }

    #[test]
    fn ends_matching_leftovers_and_spares_reused_pids() {
        let mut leftover = std::process::Command::new("sleep")
            .arg("60")
            .process_group(0)
            .spawn()
            .unwrap();
        let mut stranger = std::process::Command::new("sleep")
            .arg("60")
            .process_group(0)
            .spawn()
            .unwrap();
        let start_of = |pid: u32| sys::process(pid as i32).unwrap().start;
        let records = [
            RunRecord {
                service: 1,
                run: 1,
                runner_pid: -1,
                runner_start: 0,
                group: Some(leftover.id() as i32),
                group_start: Some(start_of(leftover.id())),
            },
            // Same PID, different start time: someone else's process now.
            RunRecord {
                service: 2,
                run: 1,
                runner_pid: -1,
                runner_start: 0,
                group: Some(stranger.id() as i32),
                group_start: Some(start_of(stranger.id()) + 1),
            },
        ];
        let report = clean_up(&records, Duration::from_secs(2));
        assert!(leftover.wait().unwrap().signal().is_some());
        assert!(stranger.try_wait().unwrap().is_none(), "never touched");
        assert!(report[1].contains("PID reused"));
        stranger.kill().unwrap();
        stranger.wait().unwrap();
    }

    use std::os::unix::process::ExitStatusExt;
}
