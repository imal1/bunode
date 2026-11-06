//! /proc, for running the core's tests on Linux.

use std::fs;

use super::ProcessInfo;

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
