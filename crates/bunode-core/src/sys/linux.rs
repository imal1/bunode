//! /proc, for running the core's tests on Linux.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use super::ProcessInfo;

pub(super) fn all_pids() -> Vec<i32> {
    fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse().ok())
        .collect()
}

pub(super) fn arguments(pid: i32) -> Option<Vec<String>> {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if raw.is_empty() {
        return None;
    }
    Some(
        raw.split(|&b| b == 0)
            .filter(|a| !a.is_empty())
            .map(|a| String::from_utf8_lossy(a).into_owned())
            .collect(),
    )
}

pub(super) fn resident_memory(pid: i32) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let kb: u64 = status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(kb * 1024)
}

pub(super) fn cwd(pid: i32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

pub(super) fn listening_ports(pid: i32) -> Vec<u16> {
    let inodes: HashSet<String> = fs::read_dir(format!("/proc/{pid}/fd"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|fd| {
            let target = fs::read_link(fd.path()).ok()?;
            let target = target.to_str()?;
            Some(
                target
                    .strip_prefix("socket:[")?
                    .strip_suffix(']')?
                    .to_owned(),
            )
        })
        .collect();
    if inodes.is_empty() {
        return Vec::new();
    }
    let mut ports = Vec::new();
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(text) = fs::read_to_string(table) else {
            continue;
        };
        for line in text.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            // local_address is fields[1] ("0100007F:1F90"), st fields[3],
            // inode fields[9]; 0A is LISTEN.
            if fields.len() > 9
                && fields[3] == "0A"
                && inodes.contains(fields[9])
                && let Some(port) = fields[1].rsplit(':').next()
                && let Ok(port) = u16::from_str_radix(port, 16)
            {
                ports.push(port);
            }
        }
    }
    ports
}

pub(super) fn process(pid: i32) -> Option<ProcessInfo> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // "pid (comm) state ppid pgrp …": the name may contain spaces and ')'.
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat[open + 1..close].to_owned();
    let fields: Vec<&str> = stat[close + 2..].split_whitespace().collect();
    // After the name: state is field 3, so field N is fields[N - 3].
    let ppid = fields.get(1)?.parse().ok()?;
    let pgid = fields.get(2)?.parse().ok()?;
    let start = fields.get(19)?.parse().ok()?;
    let uid = fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|u| u.parse().ok())?;
    Some(ProcessInfo {
        pid,
        ppid,
        pgid,
        uid,
        start,
        name,
    })
}
