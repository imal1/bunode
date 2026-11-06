//! The environment Runs start from. Apps opened from Finder or as login
//! items don't inherit the terminal's PATH (fnm, nvm, …), so Bunode asks
//! the user's login shell once (requirements §3.9).

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use tokio::process::Command;

/// Printed before `env`, so whatever the shell's startup files print is
/// not mistaken for variables.
const MARKER: &str = "__BUNODE_ENVIRONMENT_FOLLOWS__";

/// Runs `shell -l -i -c env` and returns the variables it printed. The
/// shell gets `timeout`; then it and everything it started is killed.
/// `spawn_lock` is held while spawning (see `Shared::spawn_lock`).
pub async fn capture(
    shell: &Path,
    timeout: Duration,
    spawn_lock: &Mutex<()>,
) -> Result<Vec<(OsString, OsString)>, String> {
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c"])
        .arg(format!("printf '\\n{MARKER}\\n'; /usr/bin/env -0"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true);
    let child = {
        let _guard = spawn_lock.lock().unwrap_or_else(|e| e.into_inner());
        command.spawn()
    }
    .map_err(|e| format!("couldn't start {}: {e}", shell.display()))?;
    let group = child.id().map(|pid| Pid::from_raw(pid as i32));

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => parse(&output.stdout)
            .ok_or_else(|| format!("{} printed no environment", shell.display())),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => {
            if let Some(group) = group {
                let _ = killpg(group, Signal::SIGKILL);
            }
            Err(format!(
                "{} took longer than {} s",
                shell.display(),
                timeout.as_secs()
            ))
        }
    }
}

/// Parses what follows the marker: NUL-separated `KEY=value` entries, or
/// lines if the `env` had no `-0`.
fn parse(output: &[u8]) -> Option<Vec<(OsString, OsString)>> {
    let marker = format!("\n{MARKER}\n");
    let start = find(output, marker.as_bytes())? + marker.len();
    let data = &output[start..];
    let separator = if data.contains(&0) { 0 } else { b'\n' };
    let vars: Vec<(OsString, OsString)> = data
        .split(|&b| b == separator)
        .filter_map(|entry| {
            let eq = entry.iter().position(|&b| b == b'=')?;
            if eq == 0 {
                return None;
            }
            Some((
                OsString::from_vec(entry[..eq].to_vec()),
                OsString::from_vec(entry[eq + 1..].to_vec()),
            ))
        })
        .collect();
    (!vars.is_empty()).then_some(vars)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

pub fn get<'a>(env: &'a [(OsString, OsString)], key: &str) -> Option<&'a OsStr> {
    env.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_os_str())
}

/// Finds an executable named `name` in the `PATH` of `env`.
pub fn which(env: &[(OsString, OsString)], name: &str) -> Option<PathBuf> {
    let path = get(env, "PATH")?;
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

pub fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Runs `path --version` and returns what it printed, without a leading `v`.
pub async fn version(path: &Path, spawn_lock: &Mutex<()>) -> Option<String> {
    let child = {
        let _guard = spawn_lock.lock().unwrap_or_else(|e| e.into_inner());
        Command::new(path)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .ok()?
    };
    let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .ok()?
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next()?.trim();
    (!first.is_empty()).then(|| first.trim_start_matches('v').to_owned())
}

/// The user's login shell: `$SHELL`, else zsh.
pub fn default_shell() -> PathBuf {
    std::env::var_os("SHELL")
        .filter(|s| !s.as_bytes().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/bin/zsh"))
}

/// Writes an executable script for a test. Other test threads may be
/// forking right now and briefly hold the file open for writing, which
/// makes exec fail with "Text file busy" on Linux; so it waits a moment.
#[cfg(test)]
pub(crate) fn write_test_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_nul_separated_variables_after_the_marker() {
        let output = format!(
            "Welcome!\nPATH=/fake\n\n{MARKER}\nPATH=/usr/bin:/bin\0HOME=/Users/me\0MULTI=a\nb\0"
        );
        let vars = parse(output.as_bytes()).unwrap();
        assert_eq!(get(&vars, "PATH"), Some(OsStr::new("/usr/bin:/bin")));
        assert_eq!(get(&vars, "MULTI"), Some(OsStr::new("a\nb")));
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn falls_back_to_lines_and_rejects_missing_marker() {
        let output = format!("\n{MARKER}\nA=1\nB=2\n");
        assert_eq!(parse(output.as_bytes()).unwrap().len(), 2);
        assert!(parse(b"PATH=/usr/bin\n").is_none());
    }

    #[tokio::test]
    async fn captures_from_a_real_login_shell() {
        let vars = capture(
            Path::new("/bin/sh"),
            Duration::from_secs(10),
            &Mutex::new(()),
        )
        .await
        .unwrap();
        assert!(get(&vars, "PATH").is_some());
    }

    #[tokio::test]
    async fn a_shell_that_hangs_times_out() {
        let dir = tempfile::tempdir().unwrap();
        let shell = dir.path().join("slow-shell");
        write_test_executable(&shell, "#!/bin/sh\nsleep 30\n");
        let started = std::time::Instant::now();
        let error = capture(&shell, Duration::from_millis(300), &Mutex::new(()))
            .await
            .unwrap_err();
        assert!(error.contains("took longer"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn reads_versions() {
        let node = which(&std::env::vars_os().collect::<Vec<_>>(), "node");
        if let Some(node) = node {
            let version = version(&node, &Mutex::new(())).await.unwrap();
            assert!(
                version.chars().next().unwrap().is_ascii_digit(),
                "{version}"
            );
        }
    }

    #[test]
    fn which_finds_executables_only() {
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("tool");
        fs::write(&tool, "").unwrap();
        let env = vec![(OsString::from("PATH"), dir.path().as_os_str().to_owned())];
        assert_eq!(which(&env, "tool"), None);
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(which(&env, "tool"), Some(tool));
    }
}
