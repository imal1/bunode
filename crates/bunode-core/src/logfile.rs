//! Writes each Run's full output to disk, next to the in-memory log
//! (requirements §3.4). Files roll over at a size limit and old ones are
//! deleted once a Service's logs pass a total limit. Writing happens on a
//! thread of its own and never holds up the Run: when the disk can't keep
//! up, lines are skipped and counted.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::logs::{LogBuffer, LogStream};
use crate::model::ServiceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// A file is closed and a new one started past this size.
    pub file_bytes: u64,
    /// Oldest files are deleted while a Service's logs take more than this.
    pub service_bytes: u64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            file_bytes: 5 * 1024 * 1024,
            service_bytes: 50 * 1024 * 1024,
        }
    }
}

/// Where Run logs live: `<dir>/service-<id>/run-<n>-<part>.log`.
#[derive(Debug, Clone)]
pub struct LogFiles {
    dir: PathBuf,
    limits: Limits,
}

impl LogFiles {
    pub fn new(dir: impl Into<PathBuf>, limits: Limits) -> LogFiles {
        LogFiles {
            dir: dir.into(),
            limits,
        }
    }

    pub fn service_dir(&self, service: ServiceId) -> PathBuf {
        self.dir.join(format!("service-{service}"))
    }

    fn part(&self, service: ServiceId, run: u64, part: u32) -> PathBuf {
        self.service_dir(service)
            .join(format!("run-{run}-{part:03}.log"))
    }

    /// The files of one Run, in order.
    pub fn files_of_run(&self, service: ServiceId, run: u64) -> Vec<PathBuf> {
        let prefix = format!("run-{run}-");
        let mut files: Vec<PathBuf> = fs::read_dir(self.service_dir(service))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".log"))
            })
            .collect();
        files.sort();
        files
    }

    /// Writes the Run's files, one after the other, to `destination`.
    pub fn export(&self, service: ServiceId, run: u64, destination: &Path) -> io::Result<u64> {
        let mut out = BufWriter::new(File::create(destination)?);
        let mut total = 0;
        for file in self.files_of_run(service, run) {
            total += io::copy(&mut File::open(file)?, &mut out)?;
        }
        out.flush()?;
        Ok(total)
    }

    /// Deletes the oldest files until the Service's logs fit the limit.
    /// `keep` is never deleted.
    fn prune(&self, service: ServiceId, keep: &Path) {
        let Ok(entries) = fs::read_dir(self.service_dir(service)) else {
            return;
        };
        let mut files: Vec<(u64, u32, PathBuf, u64)> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let (run, part) = parse_name(path.file_name()?.to_str()?)?;
                let size = entry.metadata().ok()?.len();
                Some((run, part, path, size))
            })
            .collect();
        files.sort();
        let mut total: u64 = files.iter().map(|f| f.3).sum();
        for (_, _, path, size) in files {
            if total <= self.limits.service_bytes {
                break;
            }
            if path == keep {
                continue;
            }
            if fs::remove_file(&path).is_ok() {
                total -= size;
            }
        }
    }

    /// Deletes every log file of a Service.
    pub fn remove_service(&self, service: ServiceId) {
        let _ = fs::remove_dir_all(self.service_dir(service));
    }
}

/// `run-14-002.log` → (14, 2)
fn parse_name(name: &str) -> Option<(u64, u32)> {
    let rest = name.strip_prefix("run-")?.strip_suffix(".log")?;
    let (run, part) = rest.rsplit_once('-')?;
    Some((run.parse().ok()?, part.parse().ok()?))
}

enum Message {
    Line {
        time_ms: i64,
        stream: LogStream,
        text: String,
    },
    Flush(mpsc::Sender<()>),
}

/// The sending side of a Run's log file. Dropping it closes the file.
#[derive(Clone)]
pub struct RunLogWriter {
    tx: SyncSender<Message>,
    skipped: Arc<Mutex<u64>>,
}

/// How many lines may wait for the disk before new ones are skipped.
const QUEUE: usize = 64 * 1024;

impl RunLogWriter {
    /// Starts the writer thread of Run `run` of `service`. Problems are
    /// reported once, as a Bunode line in `logs`.
    pub fn start(
        files: &LogFiles,
        service: ServiceId,
        run: u64,
        logs: Arc<Mutex<LogBuffer>>,
    ) -> RunLogWriter {
        let (tx, rx) = mpsc::sync_channel(QUEUE);
        let skipped = Arc::new(Mutex::new(0));
        let files = files.clone();
        let counted = skipped.clone();
        let spawned = thread::Builder::new()
            .name(format!("bunode-log-{service}-{run}"))
            .spawn(move || write_loop(&files, service, run, &rx, &logs, &counted));
        if spawned.is_err() {
            // Without a thread nothing is written; sends fail and are skipped.
        }
        RunLogWriter { tx, skipped }
    }

    /// Queues a line for the file. Never blocks.
    pub fn line(&self, time_ms: i64, stream: LogStream, text: &str) {
        let message = Message::Line {
            time_ms,
            stream,
            text: text.to_owned(),
        };
        match self.tx.try_send(message) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                *self.skipped.lock().unwrap_or_else(|e| e.into_inner()) += 1;
            }
        }
    }

    /// Waits until everything queued so far is on disk, or `timeout`.
    pub fn flush(&self, timeout: Duration) {
        let (done, wait) = mpsc::channel();
        if self.tx.send(Message::Flush(done)).is_ok() {
            let _ = wait.recv_timeout(timeout);
        }
    }

    /// Lines that were not written because the disk could not keep up.
    pub fn skipped(&self) -> u64 {
        *self.skipped.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn write_loop(
    files: &LogFiles,
    service: ServiceId,
    run: u64,
    rx: &Receiver<Message>,
    logs: &Mutex<LogBuffer>,
    skipped: &Mutex<u64>,
) {
    let mut part = 0;
    let mut writer: Option<(BufWriter<File>, u64)> = None;
    let mut failed = false;

    let report = |error: &io::Error| {
        crate::supervisor::lock_logs(logs).push_system(
            crate::supervisor::now_ms(),
            format!("Couldn't write the log to disk ({error}). It is still kept in memory."),
        );
    };

    while let Ok(message) = rx.recv() {
        let (time_ms, stream, text) = match message {
            Message::Flush(done) => {
                if let Some((file, _)) = &mut writer {
                    let _ = file.flush();
                }
                let _ = done.send(());
                continue;
            }
            Message::Line {
                time_ms,
                stream,
                text,
            } => (time_ms, stream, text),
        };
        if failed {
            *skipped.lock().unwrap_or_else(|e| e.into_inner()) += 1;
            continue;
        }
        let entry = format!("{} {} {}\n", timestamp(time_ms), stream_tag(stream), text);
        let result = (|| -> io::Result<()> {
            if writer
                .as_ref()
                .is_some_and(|(_, size)| *size >= files.limits.file_bytes)
            {
                if let Some((mut file, _)) = writer.take() {
                    file.flush()?;
                }
                part += 1;
            }
            if writer.is_none() {
                let path = files.part(service, run, part);
                fs::create_dir_all(files.service_dir(service))?;
                writer = Some((BufWriter::new(File::create(&path)?), 0));
                files.prune(service, &path);
            }
            let (file, size) = writer.as_mut().expect("opened above");
            file.write_all(entry.as_bytes())?;
            *size += entry.len() as u64;
            Ok(())
        })();
        if let Err(error) = result {
            failed = true;
            writer = None;
            report(&error);
        }
    }
    if let Some((mut file, _)) = writer {
        let skipped = *skipped.lock().unwrap_or_else(|e| e.into_inner());
        if skipped > 0 {
            let note = format!(
                "{} bunode {skipped} lines were not written to this file because the disk could not keep up.\n",
                timestamp(crate::supervisor::now_ms())
            );
            let _ = file.write_all(note.as_bytes());
        }
        let _ = file.flush();
    }
}

fn stream_tag(stream: LogStream) -> &'static str {
    match stream {
        LogStream::Stdout => "out",
        LogStream::Stderr => "err",
        LogStream::System => "bunode",
    }
}

/// `2026-11-06T15:20:01.123Z`
pub fn timestamp(time_ms: i64) -> String {
    let secs = time_ms.div_euclid(1000);
    let millis = time_ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer() -> Arc<Mutex<LogBuffer>> {
        Arc::new(Mutex::new(LogBuffer::new(100)))
    }

    #[test]
    fn formats_utc_timestamps() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(timestamp(1_793_978_401_123), "2026-11-06T15:20:01.123Z");
        assert_eq!(timestamp(951_782_400_000), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn rolls_over_and_exports_everything_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let files = LogFiles::new(
            dir.path(),
            Limits {
                file_bytes: 1_000,
                service_bytes: 1_000_000,
            },
        );
        let writer = RunLogWriter::start(&files, 7, 3, buffer());
        for i in 0..200 {
            writer.line(0, LogStream::Stdout, &format!("line {i}"));
        }
        writer.flush(Duration::from_secs(5));
        let parts = files.files_of_run(7, 3);
        assert!(parts.len() > 3, "{parts:?}");

        let out = dir.path().join("export.log");
        files.export(7, 3, &out).unwrap();
        let exported = fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = exported.lines().collect();
        assert_eq!(lines.len(), 200);
        assert_eq!(lines[0], "1970-01-01T00:00:00.000Z out line 0");
        assert!(lines[199].ends_with("line 199"));
    }

    #[test]
    fn deletes_old_files_past_the_total_limit() {
        let dir = tempfile::tempdir().unwrap();
        let files = LogFiles::new(
            dir.path(),
            Limits {
                file_bytes: 500,
                service_bytes: 2_000,
            },
        );
        for run in 1..=3 {
            let writer = RunLogWriter::start(&files, 1, run, buffer());
            for i in 0..100 {
                writer.line(0, LogStream::Stdout, &format!("run {run} line {i}"));
            }
            writer.flush(Duration::from_secs(5));
        }
        let total: u64 = fs::read_dir(files.service_dir(1))
            .unwrap()
            .flatten()
            .map(|e| e.metadata().unwrap().len())
            .sum();
        // The limit holds, give or take the file being written.
        assert!(total <= 2_000 + 600, "{total}");
        assert!(
            files.files_of_run(1, 1).is_empty(),
            "the oldest Run went first"
        );
        assert!(!files.files_of_run(1, 3).is_empty());
    }

    #[test]
    fn an_unwritable_folder_is_reported_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("not-a-folder");
        fs::write(&blocker, "").unwrap();
        let files = LogFiles::new(&blocker, Limits::default());
        let logs = buffer();
        let writer = RunLogWriter::start(&files, 1, 1, logs.clone());
        writer.line(0, LogStream::Stdout, "first");
        writer.line(0, LogStream::Stdout, "second");
        writer.flush(Duration::from_secs(5));
        let lines = logs.lock().unwrap().lines(0, 10);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].text.starts_with("Couldn't write the log to disk"));
        assert_eq!(writer.skipped(), 1);
    }
}
