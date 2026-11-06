//! UniFFI bindings that expose `bunode-core` to the Swift app.
//!
//! This is the only crate that knows about UniFFI. It stays a thin adapter:
//! no process-management logic lives here. Core types that cross the
//! boundary unchanged are re-declared with `#[uniffi::remote]`, which must
//! list the same fields as the core definition.

use std::path::PathBuf;
use std::sync::Arc;

use bunode_core::{
    AddProjectOutcome, EnvironmentState, LogColor, LogLine, LogSpan, LogStream, LogStyle,
    NewService, PackageManager, ProjectScripts, ProjectState, RunStatus, ScriptGroup, ScriptOption,
    ServiceState, SettingsState, Snapshot, Tool, ToolState,
};

uniffi::setup_scaffolding!();

/// The version of the Rust core linked into the app.
#[uniffi::export]
pub fn core_version() -> String {
    bunode_core::version().to_owned()
}

// MARK: - Types shared with the core

#[uniffi::remote(Enum)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

#[uniffi::remote(Record)]
pub struct Snapshot {
    pub projects: Vec<ProjectState>,
    pub settings: SettingsState,
    pub environment: EnvironmentState,
    pub config_issue: Option<String>,
}

#[uniffi::remote(Record)]
pub struct EnvironmentState {
    pub shell: String,
    pub variable_count: u32,
    pub loaded_at: Option<i64>,
    pub is_loading: bool,
    pub problem: Option<String>,
    pub tools: Vec<ToolState>,
}

#[uniffi::remote(Enum)]
pub enum Tool {
    Node,
    Bun,
}

#[uniffi::remote(Record)]
pub struct ToolState {
    pub tool: Tool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub is_custom: bool,
}

#[uniffi::remote(Record)]
pub struct ProjectState {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub package_manager: PackageManager,
    pub detected_package_manager: PackageManager,
    pub package_manager_choice: Option<PackageManager>,
    pub several_lockfiles: bool,
    pub services: Vec<ServiceState>,
}

#[uniffi::remote(Record)]
pub struct ServiceState {
    pub id: u64,
    pub project_id: u64,
    pub name: String,
    pub command: String,
    pub folder: Option<String>,
    pub port: Option<u16>,
    pub run_through_shell: bool,
    pub status: RunStatus,
    pub run_number: Option<u64>,
    pub pid: Option<u32>,
    pub last_exit: Option<String>,
    pub log_first: u64,
    pub log_count: u64,
    pub log_dropped: u64,
    pub log_match_first: u64,
    pub log_match_count: u64,
    pub log_filter_generation: u64,
}

#[uniffi::remote(Enum)]
pub enum RunStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Restarting,
    Crashed,
    Failed,
}

#[uniffi::remote(Record)]
pub struct SettingsState {
    pub show_running_count: bool,
    pub force_stop_after_secs: u32,
    pub log_lines_kept: u32,
}

#[uniffi::remote(Enum)]
pub enum AddProjectOutcome {
    Added { project: ProjectState },
    AlreadyAdded { project: ProjectState },
}

#[uniffi::remote(Record)]
pub struct ProjectScripts {
    pub has_package_json: bool,
    pub groups: Vec<ScriptGroup>,
}

#[uniffi::remote(Record)]
pub struct ScriptGroup {
    pub package_name: Option<String>,
    pub folder: Option<String>,
    pub scripts: Vec<ScriptOption>,
}

#[uniffi::remote(Record)]
pub struct ScriptOption {
    pub name: String,
    pub command: String,
    pub used: bool,
}

#[uniffi::remote(Record)]
pub struct NewService {
    pub name: String,
    pub command: String,
    pub folder: Option<String>,
}

#[uniffi::remote(Record)]
pub struct LogLine {
    pub seq: u64,
    pub time_ms: i64,
    pub stream: LogStream,
    pub text: String,
    pub spans: Vec<LogSpan>,
}

#[uniffi::remote(Enum)]
pub enum LogStream {
    Stdout,
    Stderr,
    System,
}

#[uniffi::remote(Record)]
pub struct LogSpan {
    pub start: u32,
    pub length: u32,
    pub style: LogStyle,
}

#[uniffi::remote(Record)]
pub struct LogStyle {
    pub foreground: Option<LogColor>,
    pub background: Option<LogColor>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

#[uniffi::remote(Enum)]
pub enum LogColor {
    Palette { index: u8 },
    Rgb { red: u8, green: u8, blue: u8 },
}

// MARK: - Errors

/// An error with a message written for the user.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{message}")]
    Failed { message: String },
}

impl From<bunode_core::Error> for CoreError {
    fn from(error: bunode_core::Error) -> Self {
        CoreError::Failed {
            message: error.to_string(),
        }
    }
}

// MARK: - The core object

/// Told when the core's state changed; the app then asks for a snapshot.
/// Called on a background thread, at most once per 100 ms.
#[uniffi::export(callback_interface)]
pub trait CoreListener: Send + Sync {
    fn state_changed(&self);
}

struct ListenerAdapter(Box<dyn CoreListener>);

impl bunode_core::Listener for ListenerAdapter {
    fn state_changed(&self) {
        self.0.state_changed();
    }
}

#[derive(uniffi::Record)]
pub struct CoreOptions {
    /// Where `config.json` lives, normally `~/Library/Application Support/Bunode`.
    pub data_dir: String,
    /// The `bunode-runner` executable inside the app bundle.
    pub runner_path: String,
}

#[derive(uniffi::Object)]
pub struct BunodeCore {
    supervisor: bunode_core::Supervisor,
}

#[uniffi::export]
impl BunodeCore {
    #[uniffi::constructor]
    pub fn new(options: CoreOptions, listener: Box<dyn CoreListener>) -> Arc<Self> {
        let supervisor = bunode_core::Supervisor::new(
            bunode_core::SupervisorOptions::new(options.data_dir, options.runner_path),
            Arc::new(ListenerAdapter(listener)),
        );
        Arc::new(BunodeCore { supervisor })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.supervisor.snapshot()
    }

    /// Registers a folder as a Project.
    pub fn add_project(&self, path: String) -> Result<AddProjectOutcome, CoreError> {
        Ok(self.supervisor.add_project(&PathBuf::from(path))?)
    }

    /// The scripts the New Service view offers, read fresh from disk.
    pub fn project_scripts(&self, project_id: u64) -> Result<ProjectScripts, CoreError> {
        Ok(self.supervisor.project_scripts(project_id)?)
    }

    pub fn add_service(
        &self,
        project_id: u64,
        service: NewService,
    ) -> Result<ServiceState, CoreError> {
        Ok(self.supervisor.add_service(project_id, service)?)
    }

    /// Starts a Run unless one is live. Returns at once.
    pub fn start(&self, service_id: u64) -> Result<(), CoreError> {
        Ok(self.supervisor.start(service_id)?)
    }

    /// Stops the Service's Run. Returns at once.
    pub fn stop(&self, service_id: u64) {
        self.supervisor.stop(service_id);
    }

    /// Force-kills the Service's Run.
    pub fn force_kill(&self, service_id: u64) {
        self.supervisor.force_kill(service_id);
    }

    /// General › Show running count.
    pub fn set_show_running_count(&self, show: bool) -> Result<(), CoreError> {
        Ok(self.supervisor.set_show_running_count(show)?)
    }

    /// Reads the login shell's environment again, in the background.
    pub fn reload_environment(&self) {
        self.supervisor.reload_environment();
    }

    /// Advanced › Node.js / Bun › Change…; `None` goes back to PATH.
    pub fn set_tool_path(&self, tool: Tool, path: Option<String>) -> Result<(), CoreError> {
        Ok(self
            .supervisor
            .set_tool_path(tool, path.as_deref().map(std::path::Path::new))?)
    }

    /// Up to `max` lines of the Service's current or last Run, from `from` on.
    pub fn log_lines(&self, service_id: u64, from: u64, max: u32) -> Vec<LogLine> {
        self.supervisor.log_lines(service_id, from, max)
    }

    /// Logs › Search; an empty query shows everything. Returns at once.
    pub fn set_log_filter(&self, service_id: u64, query: String) {
        self.supervisor.set_log_filter(service_id, &query);
    }

    /// Up to `max` lines matching the search, from match number `from` on.
    pub fn log_matching_lines(&self, service_id: u64, from: u64, max: u32) -> Vec<LogLine> {
        self.supervisor.log_matching_lines(service_id, from, max)
    }

    /// Logs › Clear: hides the lines shown so far.
    pub fn clear_logs(&self, service_id: u64) {
        self.supervisor.clear_logs(service_id);
    }

    /// Advanced › Log lines kept: 5,000, 10,000 or 50,000.
    pub fn set_log_lines_kept(&self, lines: u32) -> Result<(), CoreError> {
        Ok(self.supervisor.set_log_lines_kept(lines)?)
    }

    /// Advanced › Force stop after: 3, 5, 10 or 30 seconds.
    pub fn set_force_stop_after(&self, seconds: u32) -> Result<(), CoreError> {
        Ok(self.supervisor.set_force_stop_after(seconds)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Quiet;
    impl CoreListener for Quiet {
        fn state_changed(&self) {}
    }

    #[test]
    fn core_version_matches_core() {
        assert_eq!(core_version(), bunode_core::version());
    }

    #[test]
    fn adds_a_project_through_the_ffi_surface() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        let core = BunodeCore::new(
            CoreOptions {
                data_dir: data.path().display().to_string(),
                runner_path: "/nonexistent/bunode-runner".into(),
            },
            Box::new(Quiet),
        );
        let outcome = core.add_project(code.path().display().to_string()).unwrap();
        assert!(matches!(outcome, AddProjectOutcome::Added { .. }));
        assert_eq!(core.snapshot().projects.len(), 1);

        let error = core.add_project("/nonexistent/folder".into()).unwrap_err();
        assert!(error.to_string().contains("is not a folder"));
    }
}
