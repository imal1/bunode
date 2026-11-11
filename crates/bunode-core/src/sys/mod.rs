//! What Bunode needs to know about processes it did not necessarily start:
//! identity (to never signal a reused PID), and later discovery and ports.
//! On macOS this asks libproc directly instead of running `ps` or `lsof`
//! (requirements §2.1). Linux, through /proc, only exists so the core's
//! tests run on Linux machines too.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

/// A process, as the system reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: i32,
    pub ppid: i32,
    pub pgid: i32,
    pub uid: u32,
    /// When it started, in a unit that only means something compared with
    /// another reading of the same process. Together with the PID it says
    /// "this is still the same process".
    pub start: u64,
    /// The executable's short name, such as "node".
    pub name: String,
}

/// Every PID on the machine.
pub fn all_pids() -> Vec<i32> {
    platform::all_pids()
}

/// Every process on the machine, by PID.
pub fn processes() -> std::collections::HashMap<i32, ProcessInfo> {
    all_pids()
        .into_iter()
        .filter_map(process)
        .map(|p| (p.pid, p))
        .collect()
}

/// The process's arguments, `argv[0]` first. Only readable for the
/// user's own processes.
pub fn arguments(pid: i32) -> Option<Vec<String>> {
    platform::arguments(pid)
}

/// The process's working directory.
pub fn cwd(pid: i32) -> Option<std::path::PathBuf> {
    platform::cwd(pid)
}

/// The TCP ports the process listens on, IPv4 and IPv6, sorted.
pub fn listening_ports(pid: i32) -> Vec<u16> {
    let mut ports = platform::listening_ports(pid);
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// How much memory the process has resident, in bytes.
pub fn resident_memory(pid: i32) -> Option<u64> {
    platform::resident_memory(pid)
}

/// The user Bunode runs as.
pub fn current_uid() -> u32 {
    nix::unistd::getuid().as_raw()
}

/// The process with this PID, if there is one.
pub fn process(pid: i32) -> Option<ProcessInfo> {
    if pid <= 0 {
        return None;
    }
    platform::process(pid)
}

/// Whether `pid` still names the process that started at `start`.
pub fn is_same_process(pid: i32, start: u64) -> bool {
    process(pid).is_some_and(|p| p.start == start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_itself_and_its_start_time() {
        let me = process(std::process::id() as i32).expect("this process");
        assert_eq!(me.pid, std::process::id() as i32);
        assert!(me.start > 0);
        assert!(is_same_process(me.pid, me.start));
        assert!(!is_same_process(me.pid, me.start + 1));
        assert!(process(-1).is_none());
    }

    #[test]
    fn reads_arguments_folder_and_ports_of_a_child() {
        use std::net::TcpListener;
        let dir = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new("sleep")
            .args(["7", "extra"])
            .current_dir(dir.path())
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        assert!(all_pids().contains(&pid));
        // Right after fork the child may not have exec'd yet.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while arguments(pid).is_none_or(|a| a[0] != "sleep") && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(arguments(pid).unwrap(), ["sleep", "7", "extra"]);
        assert_eq!(cwd(pid).unwrap(), dir.path().canonicalize().unwrap());
        child.kill().unwrap();
        child.wait().unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(listening_ports(std::process::id() as i32).contains(&port));
    }

    #[test]
    fn knows_its_own_memory() {
        let bytes = resident_memory(std::process::id() as i32).unwrap();
        assert!(bytes > 1024 * 1024, "{bytes}");
    }

    #[test]
    fn a_child_has_us_as_parent() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        let info = process(child.id() as i32).unwrap();
        assert_eq!(info.ppid, std::process::id() as i32);
        assert_eq!(info.name, "sleep");
        child.kill().unwrap();
        child.wait().unwrap();
    }
}
