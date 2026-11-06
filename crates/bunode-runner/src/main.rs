//! `bunode-runner`: the per-Run helper described in ADR-0003.
//!
//! Bunode never spawns a Service's command directly. It spawns this binary,
//! which starts the command in a new process group and then watches its
//! lifeline: a Unix socket on standard input whose other end Bunode holds.
//! When the lifeline closes, because Bunode asked for a stop, quit, crashed
//! or was killed, the runner sends SIGTERM to the whole group, escalates to
//! SIGKILL after the stop timeout, and exits. When the command exits on its
//! own, the runner ends whatever is left of its group.
//!
//! The command's standard output and error are the runner's own, which
//! Bunode reads directly; its standard input is `/dev/null`.
//!
//! ```text
//! bunode-runner [--stop-timeout-ms N] [--diagnostics FILE] -- PROGRAM [ARG...]
//! ```
//!
//! With `--diagnostics`, what the runner does to the group is appended to
//! FILE (normally `~/Library/Logs/Bunode/runner.log`).
//!
//! Protocol on the lifeline, one line per message:
//!
//! - runner → Bunode: `started PID` (PID is also the process group),
//!   `spawn-failed MESSAGE`, `exited code N` or `exited signal N`.
//! - Bunode → runner: `kill` force-kills the group at once. End of file
//!   stops the group gracefully.

use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::PathBuf;
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

const DEFAULT_STOP_TIMEOUT: Duration = Duration::from_secs(5);
/// How long to wait for the group to disappear after SIGKILL.
const KILL_GRACE: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(20);

enum Event {
    /// Stop gracefully: the lifeline closed, or the runner got a signal.
    Stop,
    /// Force-kill now.
    Kill,
    /// The command (the group leader) exited.
    Exited(io::Result<ExitStatus>),
}

struct Args {
    stop_timeout: Duration,
    diagnostics: Option<PathBuf>,
    program: String,
    args: Vec<String>,
}

/// Appends what the runner does to the diagnostics file, if there is one.
struct Diagnostics(Option<PathBuf>);

impl Diagnostics {
    fn log(&self, message: &str) {
        let Some(path) = &self.0 else { return };
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
            return;
        };
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "{secs} runner {}: {message}", std::process::id());
    }
}

fn parse_args() -> Result<Args, String> {
    let mut argv = std::env::args().skip(1);
    let mut stop_timeout = DEFAULT_STOP_TIMEOUT;
    let mut diagnostics = None;
    loop {
        match argv.next().as_deref() {
            Some("--diagnostics") => {
                diagnostics = Some(PathBuf::from(
                    argv.next().ok_or("--diagnostics needs a file")?,
                ));
            }
            Some("--stop-timeout-ms") => {
                let ms = argv
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .ok_or("--stop-timeout-ms needs a number of milliseconds")?;
                stop_timeout = Duration::from_millis(ms);
            }
            Some("--") => break,
            Some(other) => return Err(format!("unexpected argument {other:?}")),
            None => return Err("missing -- PROGRAM".into()),
        }
    }
    let program = argv.next().ok_or("missing PROGRAM after --")?;
    Ok(Args {
        stop_timeout,
        diagnostics,
        program,
        args: argv.collect(),
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("bunode-runner: {message}");
            eprintln!(
                "usage: bunode-runner [--stop-timeout-ms N] [--diagnostics FILE] -- PROGRAM [ARG...]"
            );
            return ExitCode::from(2);
        }
    };

    // Our own copy of the lifeline, close-on-exec, so the command does not
    // inherit it. Standard input itself is replaced by /dev/null in the child.
    let lifeline = match io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map(UnixStream::from)
    {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("bunode-runner: no lifeline on standard input: {e}");
            return ExitCode::from(2);
        }
    };
    let mut report = Reporter(lifeline.try_clone().ok());
    let diagnostics = Diagnostics(args.diagnostics.clone());

    let child = Command::new(&args.program)
        .args(&args.args)
        .stdin(Stdio::null())
        .process_group(0)
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            let message = if e.kind() == io::ErrorKind::NotFound && !args.program.contains('/') {
                format!("{} wasn't found in PATH.", args.program)
            } else if e.kind() == io::ErrorKind::NotFound {
                format!("{} doesn't exist.", args.program)
            } else {
                format!("Couldn't start {}: {e}", args.program)
            };
            report.line(&format!("spawn-failed {message}"));
            diagnostics.log(&message);
            eprintln!("bunode-runner: {message}");
            return ExitCode::from(127);
        }
    };
    let group = Pid::from_raw(child.id() as i32);
    report.line(&format!("started {}", child.id()));
    diagnostics.log(&format!("started {} as group {}", args.program, child.id()));

    let (tx, rx) = mpsc::channel();
    {
        let tx = tx.clone();
        thread::spawn(move || {
            let status = child.wait();
            let _ = tx.send(Event::Exited(status));
        });
    }
    {
        let tx = tx.clone();
        thread::spawn(move || watch_lifeline(lifeline, &tx));
    }
    if let Ok(mut signals) = Signals::new([SIGTERM, SIGINT, SIGHUP]) {
        let tx = tx.clone();
        thread::spawn(move || {
            for _ in signals.forever() {
                if tx.send(Event::Stop).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);

    let status = supervise(group, &rx, args.stop_timeout, &diagnostics);
    end_group(group, args.stop_timeout, &diagnostics);

    match status {
        Some(Ok(status)) => {
            if let Some(code) = status.code() {
                report.line(&format!("exited code {code}"));
            } else if let Some(signal) = status.signal() {
                report.line(&format!("exited signal {signal}"));
            }
        }
        Some(Err(e)) => report.line(&format!("exited unknown {e}")),
        None => report.line("exited unknown"),
    }
    ExitCode::SUCCESS
}

/// Waits for the group leader to exit, stopping or killing the group when
/// asked. Returns the leader's exit status.
fn supervise(
    group: Pid,
    rx: &mpsc::Receiver<Event>,
    stop_timeout: Duration,
    diagnostics: &Diagnostics,
) -> Option<io::Result<ExitStatus>> {
    let mut deadline: Option<Instant> = None;
    loop {
        let event = match deadline {
            None => rx.recv().ok()?,
            Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(event) => event,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    diagnostics.log(&format!("group {group} outlived SIGTERM; SIGKILL"));
                    signal_group(group, Signal::SIGKILL);
                    deadline = Some(Instant::now() + KILL_GRACE);
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            },
        };
        match event {
            Event::Exited(status) => return Some(status),
            Event::Stop if deadline.is_none() => {
                diagnostics.log(&format!("stopping: SIGTERM to group {group}"));
                signal_group(group, Signal::SIGTERM);
                deadline = Some(Instant::now() + stop_timeout);
            }
            Event::Stop => {}
            Event::Kill => {
                diagnostics.log(&format!("force kill: SIGKILL to group {group}"));
                signal_group(group, Signal::SIGKILL);
                deadline = Some(Instant::now() + KILL_GRACE);
            }
        }
    }
}

/// After the leader exited, ends whatever is left of its group: SIGTERM,
/// then SIGKILL after the stop timeout.
fn end_group(group: Pid, stop_timeout: Duration, diagnostics: &Diagnostics) {
    if group_is_gone(group) {
        return;
    }
    diagnostics.log(&format!("command exited; ending the rest of group {group}"));
    signal_group(group, Signal::SIGTERM);
    if wait_until_gone(group, stop_timeout) {
        return;
    }
    signal_group(group, Signal::SIGKILL);
    wait_until_gone(group, KILL_GRACE);
}

fn signal_group(group: Pid, signal: Signal) {
    let _ = killpg(group, signal);
}

fn group_is_gone(group: Pid) -> bool {
    killpg(group, None) == Err(Errno::ESRCH)
}

fn wait_until_gone(group: Pid, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if group_is_gone(group) {
            return true;
        }
        thread::sleep(POLL);
    }
    group_is_gone(group)
}

/// Reads the lifeline until it closes. `kill` force-kills; end of file (or
/// any read error) stops.
fn watch_lifeline(lifeline: UnixStream, tx: &mpsc::Sender<Event>) {
    for line in BufReader::new(lifeline).lines() {
        match line {
            Ok(line) if line.trim() == "kill" => {
                if tx.send(Event::Kill).is_err() {
                    return;
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = tx.send(Event::Stop);
}

/// Writes status lines to the lifeline. Bunode may already be gone, so
/// write errors are ignored.
struct Reporter(Option<UnixStream>);

impl Reporter {
    fn line(&mut self, line: &str) {
        if let Some(stream) = &mut self.0 {
            let _ = writeln!(stream, "{line}");
        }
    }
}
