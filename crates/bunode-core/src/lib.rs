//! Bunode's process-management core.
//!
//! This crate holds the Supervisor and everything it needs: the Project
//! repository, Run state machines, Process Discovery, port scanning and the
//! log collector. It must not depend on the UI, on the FFI layer (UniFFI) or
//! on any Apple framework, so that it can later be hosted by a standalone
//! Supervisor daemon unchanged (ADR-0001, ADR-0002).

pub mod ansi;
pub mod env;
pub mod error;
pub mod launch;
pub mod logfile;
pub mod logs;
pub mod model;
pub mod package;
pub mod snapshot;
pub mod store;
pub mod supervisor;

pub use ansi::{LogColor, LogSpan, LogStyle};
pub use error::{Error, Result};
pub use logs::{LogLine, LogStream};
pub use model::PackageManager;
pub use snapshot::{
    AddProjectOutcome, EnvironmentState, NewService, ProjectScripts, ProjectState, RunStatus,
    ScriptGroup, ScriptOption, ServiceEdit, ServiceState, SettingsState, Snapshot, Tool, ToolState,
};
pub use supervisor::{
    FORCE_STOP_CHOICES, LOG_LINES_CHOICES, Listener, Supervisor, SupervisorOptions,
};

/// The version of `bunode-core`, as set in its `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "expected MAJOR.MINOR.PATCH, got {}",
            version()
        );
        assert!(
            parts.iter().all(|p| p.parse::<u32>().is_ok()),
            "expected numeric parts, got {}",
            version()
        );
    }
}
