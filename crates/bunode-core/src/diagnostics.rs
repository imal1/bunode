//! The Supervisor's own log, `~/Library/Logs/Bunode/supervisor.log`, for
//! finding out afterwards what happened (Advanced › Diagnostics).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Past this size the log starts over, keeping one older file.
const MAX_BYTES: u64 = 5 * 1024 * 1024;

pub struct Diagnostics {
    file: Mutex<Option<File>>,
}

impl Diagnostics {
    /// Logs to `<dir>/<name>`, or nowhere when `dir` is `None`.
    pub fn open(dir: Option<&Path>, name: &str) -> Diagnostics {
        let file = dir.and_then(|dir| {
            fs::create_dir_all(dir).ok()?;
            let path = dir.join(name);
            if path.metadata().is_ok_and(|m| m.len() > MAX_BYTES) {
                let _ = fs::rename(&path, old_path(&path));
            }
            OpenOptions::new().create(true).append(true).open(path).ok()
        });
        Diagnostics {
            file: Mutex::new(file),
        }
    }

    pub fn log(&self, message: &str) {
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(file) = file.as_mut() {
            let line = format!(
                "{} {message}\n",
                crate::logfile::timestamp(crate::supervisor::now_ms())
            );
            let _ = file.write_all(line.as_bytes());
        }
    }
}

fn old_path(path: &Path) -> PathBuf {
    path.with_extension("old.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_timestamped_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log = Diagnostics::open(Some(dir.path()), "supervisor.log");
        log.log("started");
        log.log("stopped");
        let text = fs::read_to_string(dir.path().join("supervisor.log")).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("Z started"));
        Diagnostics::open(None, "x").log("goes nowhere");
    }
}
