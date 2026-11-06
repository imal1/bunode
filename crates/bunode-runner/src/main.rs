//! `bunode-runner`: the per-Run helper described in ADR-0003.
//!
//! Bunode will never spawn a Service's command directly. It will spawn this
//! binary, which creates the process group, starts the command inside it and
//! holds a lifeline pipe to the app; when the pipe closes for any reason, it
//! sends SIGTERM to the group, escalates to SIGKILL after the timeout, and
//! exits.
//!
//! For now it is an empty binary, so the workspace and the build already
//! have their final shape.

fn main() {}
