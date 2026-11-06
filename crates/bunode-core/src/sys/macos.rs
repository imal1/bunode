//! libproc, which is part of libSystem (not a framework).
//!
//! The only unsafe code of the core: calls into libproc with buffers that
//! are sized and zeroed here, whose returned length is always checked.
#![allow(unsafe_code)]

use std::ffi::{CStr, c_int, c_void};
use std::mem::{size_of, zeroed};

use libc::{PROC_PIDTBSDINFO, proc_bsdinfo, proc_pidinfo};

use super::ProcessInfo;

fn bsd_info(pid: i32) -> Option<proc_bsdinfo> {
    // SAFETY: proc_bsdinfo is plain data, so all-zero is a valid value, and
    // proc_pidinfo writes at most `size` bytes into it.
    let mut info: proc_bsdinfo = unsafe { zeroed() };
    let size = size_of::<proc_bsdinfo>() as c_int;
    let written = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut proc_bsdinfo).cast::<c_void>(),
            size,
        )
    };
    (written == size).then_some(info)
}

pub(super) fn process(pid: i32) -> Option<ProcessInfo> {
    let info = bsd_info(pid)?;
    // SAFETY: pbi_comm is a NUL-terminated C array filled by the kernel;
    // the last byte is forced to NUL in case it is not.
    let mut comm = info.pbi_comm;
    if let Some(last) = comm.last_mut() {
        *last = 0;
    }
    let name = unsafe { CStr::from_ptr(comm.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    Some(ProcessInfo {
        pid: info.pbi_pid as i32,
        ppid: info.pbi_ppid as i32,
        pgid: info.pbi_pgid as i32,
        uid: info.pbi_uid,
        start: info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec,
        name,
    })
}
