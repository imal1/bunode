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
