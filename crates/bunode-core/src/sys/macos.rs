//! libproc, which is part of libSystem (not a framework).
//!
//! The only unsafe code of the core: calls into libproc with buffers that
//! are sized and zeroed here, whose returned length is always checked.
#![allow(unsafe_code)]

use std::ffi::{CStr, OsStr, c_int, c_void};
use std::mem::{size_of, zeroed};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;

use libc::{
    CTL_KERN, KERN_ARGMAX, KERN_PROCARGS2, PROC_PIDLISTFDS, PROC_PIDTBSDINFO,
    PROC_PIDVNODEPATHINFO, PROX_FDTYPE_SOCKET, proc_bsdinfo, proc_fdinfo, proc_listallpids,
    proc_pidfdinfo, proc_pidinfo, proc_vnodepathinfo, sysctl,
};

use super::ProcessInfo;

pub(super) fn all_pids() -> Vec<i32> {
    // SAFETY: with a null buffer proc_listallpids only returns the count.
    let count = unsafe { proc_listallpids(ptr::null_mut(), 0) };
    if count <= 0 {
        return Vec::new();
    }
    // Room for processes started since.
    let mut pids = vec![0i32; count as usize + 128];
    let bytes = (pids.len() * size_of::<i32>()) as c_int;
    // SAFETY: the buffer holds `bytes` bytes; the call returns how many
    // PIDs it wrote, never more than fit.
    let written = unsafe { proc_listallpids(pids.as_mut_ptr().cast::<c_void>(), bytes) };
    pids.truncate(written.max(0) as usize);
    pids.retain(|&p| p > 0);
    pids
}

/// `sysctl` into a byte buffer of `size` bytes.
fn sysctl_bytes(mib: &mut [c_int], size: usize) -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; size];
    let mut length = size;
    // SAFETY: `buffer` holds `length` bytes, and sysctl updates `length`
    // to what it wrote.
    let result = unsafe {
        sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            buffer.as_mut_ptr().cast::<c_void>(),
            &mut length,
            ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return None;
    }
    buffer.truncate(length);
    Some(buffer)
}

pub(super) fn arguments(pid: i32) -> Option<Vec<String>> {
    let arg_max = sysctl_bytes(&mut [CTL_KERN, KERN_ARGMAX], size_of::<c_int>())?;
    let arg_max = c_int::from_ne_bytes(arg_max.get(..4)?.try_into().ok()?).max(4096) as usize;
    let raw = sysctl_bytes(&mut [CTL_KERN, KERN_PROCARGS2, pid], arg_max)?;
    // Layout: argc, the executable path, NUL padding, then argc arguments.
    let argc = c_int::from_ne_bytes(raw.get(..4)?.try_into().ok()?) as usize;
    let mut rest = &raw[4..];
    let path_end = rest.iter().position(|&b| b == 0)?;
    rest = &rest[path_end..];
    let start = rest.iter().position(|&b| b != 0)?;
    rest = &rest[start..];
    // A process that rewrote its title (Node does, e.g. "npm run dev")
    // leaves the title and then NULs where its arguments were: the empty
    // ones are dropped.
    let args: Vec<String> = rest
        .split(|&b| b == 0)
        .take(argc)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect();
    (!args.is_empty()).then_some(args)
}

pub(super) fn cwd(pid: i32) -> Option<PathBuf> {
    // SAFETY: plain data, zero is valid; the length is checked below.
    let mut info: proc_vnodepathinfo = unsafe { zeroed() };
    let size = size_of::<proc_vnodepathinfo>() as c_int;
    let written = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDVNODEPATHINFO,
            0,
            (&mut info as *mut proc_vnodepathinfo).cast::<c_void>(),
            size,
        )
    };
    if written != size {
        return None;
    }
    let path = &info.pvi_cdir.vip_path;
    // SAFETY: vip_path is a C char array; read up to its first NUL, within it.
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            path.as_ptr().cast::<u8>(),
            path.len() * size_of_val(&path[0]),
        )
    };
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    (end > 0).then(|| PathBuf::from(OsStr::from_bytes(&bytes[..end])))
}

/// `PROC_PIDFDSOCKETINFO`, and the parts of `struct socket_fdinfo`
/// (<sys/proc_info.h>) that are read here. libc doesn't have the struct;
/// the offsets follow its layout: a 24-byte `proc_fileinfo`, then
/// `socket_info`, whose protocol union starts 240 bytes in.
const PROC_PIDFDSOCKETINFO: c_int = 3;
const SOCKET_FDINFO_SIZE: usize = 792;
const SOI_FAMILY: usize = 24 + 160;
const SOI_KIND: usize = 24 + 232;
const SOI_PROTO: usize = 24 + 240;
/// In `tcp_sockinfo`: `tcpsi_ini.insi_lport`, then `tcpsi_state`.
const TCP_LOCAL_PORT: usize = SOI_PROTO + 4;
const TCP_STATE: usize = SOI_PROTO + 80;
const SOCKINFO_TCP: i32 = 2;
const TSI_S_LISTEN: i32 = 1;

fn read_i32(buffer: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_ne_bytes(buffer.get(at..at + 4)?.try_into().ok()?))
}

pub(super) fn listening_ports(pid: i32) -> Vec<u16> {
    // SAFETY: with a null buffer PROC_PIDLISTFDS returns the bytes needed.
    let needed = unsafe { proc_pidinfo(pid, PROC_PIDLISTFDS, 0, ptr::null_mut(), 0) };
    if needed <= 0 {
        return Vec::new();
    }
    let slots = needed as usize / size_of::<proc_fdinfo>() + 32;
    let mut fds: Vec<proc_fdinfo> = vec![unsafe { zeroed() }; slots];
    let bytes = (slots * size_of::<proc_fdinfo>()) as c_int;
    // SAFETY: `fds` holds `bytes` bytes; the call returns how many it wrote.
    let written = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDLISTFDS,
            0,
            fds.as_mut_ptr().cast::<c_void>(),
            bytes,
        )
    };
    if written <= 0 {
        return Vec::new();
    }
    fds.truncate(written as usize / size_of::<proc_fdinfo>());

    let mut ports = Vec::new();
    for fd in fds
        .iter()
        .filter(|fd| fd.proc_fdtype == PROX_FDTYPE_SOCKET as u32)
    {
        // Bigger than the struct, in case a future macOS grows it.
        let mut buffer = vec![0u8; 1024];
        // SAFETY: `buffer` holds 1024 bytes; the call writes at most that
        // and returns how many it wrote.
        let written = unsafe {
            proc_pidfdinfo(
                pid,
                fd.proc_fd,
                PROC_PIDFDSOCKETINFO,
                buffer.as_mut_ptr().cast::<c_void>(),
                buffer.len() as c_int,
            )
        };
        if (written as usize) < SOCKET_FDINFO_SIZE {
            continue;
        }
        let family = read_i32(&buffer, SOI_FAMILY);
        let is_inet = family == Some(libc::AF_INET) || family == Some(libc::AF_INET6);
        if !is_inet || read_i32(&buffer, SOI_KIND) != Some(SOCKINFO_TCP) {
            continue;
        }
        if read_i32(&buffer, TCP_STATE) != Some(TSI_S_LISTEN) {
            continue;
        }
        if let Some(port) = read_i32(&buffer, TCP_LOCAL_PORT) {
            // Stored in network byte order in the low 16 bits.
            let port = u16::from_be((port as u32 & 0xffff) as u16);
            if port != 0 {
                ports.push(port);
            }
        }
    }
    ports
}

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
    let comm = unsafe { CStr::from_ptr(comm.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    // pbi_name is the longer name (32 bytes); pbi_comm is cut at 16.
    let mut long = info.pbi_name;
    if let Some(last) = long.last_mut() {
        *last = 0;
    }
    // SAFETY: as above, NUL-terminated within the array.
    let long = unsafe { CStr::from_ptr(long.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    let name = if long.is_empty() { comm } else { long };
    Some(ProcessInfo {
        pid: info.pbi_pid as i32,
        ppid: info.pbi_ppid as i32,
        pgid: info.pbi_pgid as i32,
        uid: info.pbi_uid,
        start: info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec,
        name,
    })
}
