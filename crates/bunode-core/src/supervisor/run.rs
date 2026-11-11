//! Drives one Run: spawns `bunode-runner` with a lifeline, follows what it
//! reports, and passes stop and kill requests on (ADR-0003).

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream as StdUnixStream;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use nix::sys::signal::Signal;
use nix::unistd::Pid;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Instant, sleep_until};

use super::{RunLog, Shared};
use crate::ansi::LineParser;
use crate::launch::LaunchSpec;
use crate::logs::LogStream;
use crate::model::ServiceId;
use crate::snapshot::{RunNote, RunStatus};
use crate::tree::RunTree;

/// Longer lines are split; nobody reads a 64 KB line in a log view.
const MAX_LINE_BYTES: usize = 16 * 1024;
/// After the runner exits, how long its output gets to drain. A process
/// that escaped the group may hold the pipes open forever.
const DRAIN_GRACE: Duration = Duration::from_secs(1);

/// After a force kill, how long the runner gets to report before it is
/// killed too.
const RUNNER_KILL_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    /// SIGTERM the group, SIGKILL after the stop timeout.
    Stop,
    /// SIGKILL the group now.
    Kill,
}

/// How a Run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The command never started.
    SpawnFailed(RunNote),
    ExitedWithCode(i32),
    KilledBySignal(i32),
    /// The runner went away without saying how the command ended.
    Lost(RunNote),
}

impl Outcome {
    /// The status and note a Run ends with. Whatever happens after the user
    /// asked for a stop counts as stopped.
    pub(crate) fn classify(&self, stop_requested: bool) -> (RunStatus, Option<RunNote>) {
        match self {
            Outcome::SpawnFailed(note) => (RunStatus::Failed, Some(note.clone())),
            _ if stop_requested => (RunStatus::Stopped, None),
            Outcome::ExitedWithCode(0) => (RunStatus::Stopped, Some(RunNote::Exited)),
            Outcome::ExitedWithCode(code) => (
                RunStatus::Crashed,
                Some(RunNote::ExitedWithCode { code: *code }),
            ),
            Outcome::KilledBySignal(signal) => (
                RunStatus::Crashed,
                Some(RunNote::EndedBySignal {
                    signal: signal_name(*signal),
                }),
            ),
            Outcome::Lost(note) => (RunStatus::Crashed, Some(note.clone())),
        }
    }
}

fn signal_name(signal: i32) -> String {
    Signal::try_from(signal)
        .map(|s| s.as_str().to_owned())
        .unwrap_or_else(|_| format!("signal {signal}"))
}

/// A line the runner writes on the lifeline.
#[derive(Debug, PartialEq, Eq)]
enum Report {
    Started(u32),
    SpawnFailed(SpawnFailure),
    Exited(Outcome),
}

/// Why the runner couldn't start the command: `spawn-failed not-found` or
/// `spawn-failed error REASON`.
#[derive(Debug, PartialEq, Eq)]
enum SpawnFailure {
    NotFound,
    Other(String),
}

impl SpawnFailure {
    fn note(self, program: &str) -> RunNote {
        let program = program.to_owned();
        match self {
            SpawnFailure::NotFound if program.contains('/') => RunNote::ProgramMissing { program },
            SpawnFailure::NotFound => RunNote::NotInPath { program },
            SpawnFailure::Other(reason) => RunNote::CouldNotStart { program, reason },
        }
    }
}

fn parse_report(line: &str) -> Option<Report> {
    let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
    match kind {
        "started" => rest.trim().parse().ok().map(Report::Started),
        "spawn-failed" => Some(Report::SpawnFailed(match rest.split_once(' ') {
            _ if rest == "not-found" => SpawnFailure::NotFound,
            Some(("error", reason)) => SpawnFailure::Other(reason.to_owned()),
            _ => SpawnFailure::Other(rest.to_owned()),
        })),
        "exited" => {
            let (how, value) = rest.split_once(' ').unwrap_or((rest, ""));
            let outcome = match (how, value.trim().parse::<i32>()) {
                ("code", Ok(code)) => Outcome::ExitedWithCode(code),
                ("signal", Ok(signal)) => Outcome::KilledBySignal(signal),
                _ => Outcome::Lost(RunNote::UnknownEnd),
            };
            Some(Report::Exited(outcome))
        }
        _ => None,
    }
}

pub(crate) async fn drive(
    shared: Arc<Shared>,
    service_id: ServiceId,
    run_number: u64,
    spec: LaunchSpec,
    log: RunLog,
    mut control: UnboundedReceiver<Control>,
) -> Outcome {
    if !spec.cwd.is_dir() {
        return Outcome::SpawnFailed(RunNote::FolderMissing {
            path: spec.cwd.display().to_string(),
        });
    }

    let spawned = {
        let _guard = shared.spawn_lock.lock().unwrap_or_else(|e| e.into_inner());
        spawn_runner(
            &shared.runner_path,
            &spec,
            shared.runner_diagnostics.as_deref(),
        )
    };
    let (mut runner, lifeline) = match spawned {
        Ok(spawned) => spawned,
        Err(e) => {
            return Outcome::SpawnFailed(RunNote::RunnerFailed {
                reason: e.to_string(),
            });
        }
    };
    if let Some(pid) = runner.id() {
        shared.runner_spawned(service_id, run_number, pid);
    }

    // Output must always be drained, or a chatty Run blocks on a full pipe.
    let mut collectors = Vec::new();
    if let Some(stdout) = runner.stdout.take() {
        collectors.push(tokio::spawn(collect(
            stdout,
            LogStream::Stdout,
            log.clone(),
            shared.clone(),
        )));
    }
    if let Some(stderr) = runner.stderr.take() {
        collectors.push(tokio::spawn(collect(
            stderr,
            LogStream::Stderr,
            log.clone(),
            shared.clone(),
        )));
    }

    let (read, mut write) = lifeline.into_split();
    let mut reports = BufReader::new(read).lines();
    let mut group: Option<Pid> = None;
    let mut outcome: Option<Outcome> = None;
    let mut controls_open = true;
    // Set by a force kill: if the runner hasn't gone by then, it is killed.
    let mut runner_deadline: Option<Instant> = None;

    loop {
        tokio::select! {
            line = reports.next_line() => match line {
                Ok(Some(line)) => match parse_report(&line) {
                    Some(Report::Started(pid)) => {
                        group = Some(Pid::from_raw(pid as i32));
                        shared.started(service_id, run_number, pid);
                    }
                    Some(Report::SpawnFailed(failure)) => {
                        outcome = Some(Outcome::SpawnFailed(failure.note(&spec.program)));
                    }
                    Some(Report::Exited(exit)) => outcome = Some(exit),
                    None => {}
                },
                // The runner closed the lifeline: it is exiting.
                _ => break,
            },
            request = control.recv(), if controls_open => match request {
                // Not end of file: the lifeline stays open for a `kill`.
                Some(Control::Stop) => {
                    let _ = write.write_all(b"stop\n").await;
                }
                Some(Control::Kill) => {
                    let _ = write.write_all(b"kill\n").await;
                    if let Some(group) = group {
                        RunTree::new(group.as_raw()).signal(Signal::SIGKILL);
                    }
                    runner_deadline.get_or_insert(Instant::now() + RUNNER_KILL_GRACE);
                }
                // The Supervisor is going away: closing the lifeline stops the Run.
                None => {
                    controls_open = false;
                    let _ = write.shutdown().await;
                }
            },
            // The runner itself is not responding: end it, and the group.
            () = sleep_until(runner_deadline.unwrap_or_else(Instant::now)), if runner_deadline.is_some() => {
                runner_deadline = None;
                let _ = runner.start_kill();
                if let Some(group) = group {
                    RunTree::new(group.as_raw()).signal(Signal::SIGKILL);
                }
            },
        }
    }

    let status = runner.wait().await;
    // The runner normally leaves nothing behind. If it was killed itself,
    // end the Run here; a process group's ID is never reused while the
    // group exists, so signalling it is safe.
    if let Some(group) = group {
        let _ = end_group(group, spec.stop_timeout).await;
    }
    for collector in collectors {
        let _ = tokio::time::timeout(DRAIN_GRACE, collector).await;
    }
    outcome.unwrap_or_else(|| match status {
        Ok(status) => Outcome::Lost(RunNote::RunnerExited {
            status: status.to_string(),
        }),
        Err(e) => Outcome::Lost(RunNote::RunnerLost {
            reason: e.to_string(),
        }),
    })
}

/// Ends a process group and its descendants outside it (ADR-0010):
/// SIGTERM, then SIGKILL after `timeout`. Returns whether all of it is
/// gone, waiting up to a second after SIGKILL.
pub(super) async fn end_group(group: Pid, timeout: Duration) -> bool {
    let mut tree = RunTree::new(group.as_raw());
    tree.refresh();
    if tree.is_gone() {
        return true;
    }
    tree.signal(Signal::SIGTERM);
    let deadline = Instant::now() + timeout;
    while !tree.is_gone() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    if !tree.is_gone() {
        tree.signal(Signal::SIGKILL);
        let deadline = Instant::now() + Duration::from_secs(1);
        while !tree.is_gone() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    tree.is_gone()
}

/// Spawns the runner with a fresh lifeline on its standard input.
fn spawn_runner(
    runner_path: &std::path::Path,
    spec: &LaunchSpec,
    diagnostics: Option<&std::path::Path>,
) -> std::io::Result<(tokio::process::Child, UnixStream)> {
    let (ours, theirs) = StdUnixStream::pair()?;
    let mut command = Command::new(runner_path);
    command
        .arg("--stop-timeout-ms")
        .arg(spec.stop_timeout.as_millis().to_string());
    if let Some(diagnostics) = diagnostics {
        command.arg("--diagnostics").arg(diagnostics);
    }
    command
        .arg("--")
        .arg(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .env_clear()
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::from(OwnedFd::from(theirs)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The runner gets its own group, so signals meant for Bunode's
        // group (a terminal's Ctrl-C) don't reach it.
        .process_group(0);
    let child = command.spawn()?;
    drop(command);
    ours.set_nonblocking(true)?;
    Ok((child, UnixStream::from_std(ours)?))
}

/// Reads one output stream into the Run's log, line by line.
async fn collect(
    stream: impl AsyncRead + Unpin,
    kind: LogStream,
    log: RunLog,
    shared: Arc<Shared>,
) {
    let mut reader = BufReader::with_capacity(64 * 1024, stream);
    let mut parser = LineParser::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        match read_line(&mut reader, &mut line, MAX_LINE_BYTES).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        // The snapshot shows no lines, only ports an announcement may reorder.
        if log.line(kind, &parser.line(&line)) {
            shared.notifier.notify();
        }
    }
}

/// Reads up to and including the next newline, or `max` bytes, whichever
/// comes first. Returns how many bytes were read; 0 at the end.
async fn read_line(
    reader: &mut (impl AsyncBufRead + Unpin),
    out: &mut Vec<u8>,
    max: usize,
) -> std::io::Result<usize> {
    let mut read = 0;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(read);
        }
        let room = max - out.len();
        let window = &available[..available.len().min(room)];
        if let Some(end) = window.iter().position(|&b| b == b'\n') {
            out.extend_from_slice(&window[..=end]);
            reader.consume(end + 1);
            return Ok(read + end + 1);
        }
        let taken = window.len();
        out.extend_from_slice(window);
        reader.consume(taken);
        read += taken;
        if out.len() >= max {
            return Ok(read);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::signal::killpg;
    use std::os::unix::process::CommandExt;

    #[test]
    fn parses_runner_reports() {
        assert_eq!(parse_report("started 4812"), Some(Report::Started(4812)));
        assert_eq!(
            parse_report("spawn-failed not-found"),
            Some(Report::SpawnFailed(SpawnFailure::NotFound))
        );
        assert_eq!(
            parse_report("spawn-failed error Permission denied (os error 13)"),
            Some(Report::SpawnFailed(SpawnFailure::Other(
                "Permission denied (os error 13)".into()
            )))
        );
        assert_eq!(
            parse_report("exited code 1"),
            Some(Report::Exited(Outcome::ExitedWithCode(1)))
        );
        assert_eq!(
            parse_report("exited signal 15"),
            Some(Report::Exited(Outcome::KilledBySignal(15)))
        );
        assert_eq!(
            parse_report("exited unknown"),
            Some(Report::Exited(Outcome::Lost(RunNote::UnknownEnd)))
        );
        assert_eq!(parse_report("hello"), None);
    }

    #[test]
    fn classifies_how_runs_end() {
        use RunStatus::*;
        let stopped = |o: Outcome| o.classify(true).0;
        let unasked = |o: Outcome| o.classify(false);
        assert_eq!(stopped(Outcome::KilledBySignal(15)), Stopped);
        assert_eq!(stopped(Outcome::ExitedWithCode(143)), Stopped);
        assert_eq!(stopped(Outcome::SpawnFailed(RunNote::EmptyCommand)), Failed);
        assert_eq!(unasked(Outcome::ExitedWithCode(0)).0, Stopped);
        assert_eq!(
            unasked(Outcome::ExitedWithCode(1)),
            (Crashed, Some(RunNote::ExitedWithCode { code: 1 }))
        );
        assert_eq!(
            unasked(Outcome::KilledBySignal(9)),
            (
                Crashed,
                Some(RunNote::EndedBySignal {
                    signal: "SIGKILL".into()
                })
            )
        );
    }

    #[test]
    fn names_spawn_failures_after_the_program() {
        assert_eq!(
            SpawnFailure::NotFound.note("vite"),
            RunNote::NotInPath {
                program: "vite".into()
            }
        );
        assert_eq!(
            SpawnFailure::NotFound.note("./bin/dev"),
            RunNote::ProgramMissing {
                program: "./bin/dev".into()
            }
        );
        assert_eq!(
            SpawnFailure::Other("Permission denied".into()).note("x"),
            RunNote::CouldNotStart {
                program: "x".into(),
                reason: "Permission denied".into()
            }
        );
    }

    #[tokio::test]
    async fn end_group_ends_a_group_with_sigterm_alone() {
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .process_group(0)
            .spawn()
            .unwrap();
        let group = Pid::from_raw(child.id() as i32);
        let reaper = std::thread::spawn(move || child.wait());

        let started = std::time::Instant::now();
        assert!(end_group(group, Duration::from_secs(5)).await);
        assert!(started.elapsed() < Duration::from_secs(2));
        reaper.join().unwrap().unwrap();
    }

    #[tokio::test]
    async fn end_group_kills_a_group_that_ignores_sigterm() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "trap '' TERM; while :; do sleep 1; done"])
            .process_group(0)
            .spawn()
            .unwrap();
        let group = Pid::from_raw(child.id() as i32);
        let reaper = std::thread::spawn(move || child.wait());
        tokio::time::sleep(Duration::from_millis(200)).await;

        let started = std::time::Instant::now();
        assert!(end_group(group, Duration::from_millis(300)).await);
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert!(killpg(group, None).is_err());
        reaper.join().unwrap().unwrap();
    }
}
