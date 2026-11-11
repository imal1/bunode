//! UniFFI bindings that expose `bunode-core` to the Swift app.
//!
//! This is the only crate that knows about UniFFI. It stays a thin adapter:
//! no process-management logic lives here. Core types that cross the
//! boundary unchanged are re-declared with `#[uniffi::remote]`, which must
//! list the same fields as the core definition.

use std::path::PathBuf;
use std::sync::Arc;

use bunode_core::{
    AddProjectOutcome, AgentServerState, ConfigIssue, ConfigProblem, DiscoveredProcess,
    EnvironmentProblem, EnvironmentState, ListLayout, LogColor, LogLine, LogSpan, LogStream,
    LogStyle, NewService, PackageManager, PortConflictOwner, ProjectScripts, ProjectState, RunEnd,
    RunNote, RunStatus, ScriptGroup, ScriptOption, ServiceEdit, ServiceState, SettingsState,
    Snapshot, StartOutcome, Tool, ToolState,
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
    pub config_issue: Option<ConfigIssue>,
    pub discovered: Vec<DiscoveredProcess>,
}

#[uniffi::remote(Record)]
pub struct ConfigIssue {
    pub path: String,
    pub backup: Option<String>,
    pub problem: ConfigProblem,
}

#[uniffi::remote(Enum)]
pub enum ListLayout {
    Flat,
    ByProject,
}

#[uniffi::remote(Enum)]
pub enum ConfigProblem {
    Unreadable { detail: String },
    NewerFormat { version: u32 },
}

#[uniffi::remote(Record)]
pub struct DiscoveredProcess {
    pub pid: u32,
    pub start: u64,
    pub command: String,
    pub cwd: Option<String>,
    pub source: Option<String>,
    pub members: Vec<u32>,
    pub member_starts: Vec<u64>,
    pub ports: Vec<u16>,
    pub exiting: bool,
}

#[uniffi::remote(Record)]
pub struct EnvironmentState {
    pub shell: String,
    pub variable_count: u32,
    pub loaded_at: Option<i64>,
    pub is_loading: bool,
    pub problem: Option<EnvironmentProblem>,
    pub tools: Vec<ToolState>,
}

#[uniffi::remote(Enum)]
pub enum EnvironmentProblem {
    ShellFailed { shell: String, reason: String },
    NoOutput { shell: String },
    TimedOut { shell: String, seconds: u32 },
    ReadFailed { reason: String },
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
    pub folder_name: String,
    pub path: String,
    pub package_manager: PackageManager,
    pub detected_package_manager: PackageManager,
    pub package_manager_choice: Option<PackageManager>,
    pub several_lockfiles: bool,
    pub is_missing: bool,
    pub external: Vec<u32>,
    pub services: Vec<ServiceState>,
    pub agent_servers: Vec<AgentServerState>,
}

#[uniffi::remote(Record)]
pub struct AgentServerState {
    pub project_id: u64,
    pub agent: String,
    pub name: String,
    pub command: String,
    pub pid: u32,
    pub pgid: u32,
    pub live_ports: Vec<u16>,
    pub stopping: bool,
}

#[uniffi::remote(Record)]
pub struct ServiceState {
    pub id: u64,
    pub project_id: u64,
    pub name: String,
    pub display_name: String,
    pub command: String,
    pub folder: Option<String>,
    pub port: Option<u16>,
    pub run_through_shell: bool,
    pub status: RunStatus,
    pub run_number: Option<u64>,
    pub pid: Option<u32>,
    pub last_exit: Option<RunNote>,
    pub log_first: u64,
    pub log_count: u64,
    pub log_dropped: u64,
    pub log_match_first: u64,
    pub log_match_count: u64,
    pub log_filter_generation: u64,
    pub has_pending_config: bool,
    pub ended_by: Option<RunEnd>,
    pub live_ports: Vec<u16>,
}

#[uniffi::remote(Enum)]
pub enum RunNote {
    Exited,
    ExitedWithCode { code: i32 },
    EndedBySignal { signal: String },
    UnmatchedQuote,
    EmptyCommand,
    FolderMissing { path: String },
    NotInPath { program: String },
    ProgramMissing { program: String },
    CouldNotStart { program: String, reason: String },
    RunnerFailed { reason: String },
    RunnerExited { status: String },
    RunnerLost { reason: String },
    UnknownEnd,
}

#[uniffi::remote(Enum)]
pub enum RunEnd {
    StoppedByUser,
    BunodeQuit,
    Exited,
    Crashed,
    FailedToStart,
}

#[uniffi::remote(Record)]
pub struct ServiceEdit {
    pub command: String,
    pub folder: Option<String>,
    pub port: Option<u16>,
    pub run_through_shell: bool,
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
    pub list_layout: ListLayout,
    pub show_agent_servers: bool,
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

#[uniffi::remote(Enum)]
pub enum StartOutcome {
    Started,
    PortInUse {
        port: u16,
        owner: Option<PortConflictOwner>,
        suggestion: Option<u16>,
    },
}

#[uniffi::remote(Record)]
pub struct PortConflictOwner {
    pub pid: u32,
    pub command: String,
    pub source: Option<String>,
    pub service_id: Option<u64>,
    pub agent_server: bool,
}

// MARK: - Errors

/// `bunode_core::Error` for Swift, which words it (requirements §4.1).
/// `Display` is the core's English, for logs.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{path} is not a folder.")]
    NotAFolder { path: String },
    #[error("{path} doesn't exist.")]
    Missing { path: String },
    #[error("{path} is already a Project.")]
    AlreadyAProject { path: String },
    #[error("Choose the Project's folder or a folder inside it.")]
    OutsideProject,
    #[error("Couldn't save {path}: {reason}")]
    Save { path: String, reason: String },
    #[error("This Project is no longer in Bunode.")]
    UnknownProject,
    #[error("This Service is no longer in Bunode.")]
    UnknownService,
    #[error("Enter a command.")]
    EmptyCommand,
    #[error("The command has an unmatched quote.")]
    UnmatchedQuote,
    #[error("Use a port from 1 to 65535.")]
    InvalidPort,
    #[error("Enter a name.")]
    EmptyName,
    #[error("Use at most {max} characters.")]
    NameTooLong { max: u32 },
    #[error("Name already used in {project}")]
    NameTaken { project: String },
    #[error("{path} is not an executable file.")]
    NotExecutable { path: String },
    #[error("Force stop after must be one of {choices:?} seconds.")]
    InvalidForceStopAfter { choices: Vec<u32> },
    #[error("Log lines kept must be one of {choices:?}.")]
    InvalidLogLinesKept { choices: Vec<u32> },
    #[error("This Service has no Runs yet.")]
    NoRuns,
    #[error("Couldn't export to {path}: {reason}")]
    ExportFailed { path: String, reason: String },
    #[error("PID {pid} has already exited.")]
    ProcessExited { pid: u32 },
    #[error("Couldn't send SIGTERM to PID {pid}: {reason}")]
    SignalFailed { pid: u32, reason: String },
    #[error("Couldn’t stop {name}.")]
    AgentServerSurvived { name: String },
}

impl From<bunode_core::Error> for CoreError {
    fn from(error: bunode_core::Error) -> Self {
        use bunode_core::Error as E;
        match error {
            E::NotAFolder { path } => CoreError::NotAFolder { path },
            E::Missing { path } => CoreError::Missing { path },
            E::AlreadyAProject { path } => CoreError::AlreadyAProject { path },
            E::OutsideProject => CoreError::OutsideProject,
            E::Save { path, source } => CoreError::Save {
                path,
                reason: source.to_string(),
            },
            E::UnknownProject => CoreError::UnknownProject,
            E::UnknownService => CoreError::UnknownService,
            E::EmptyCommand => CoreError::EmptyCommand,
            E::UnmatchedQuote => CoreError::UnmatchedQuote,
            E::InvalidPort => CoreError::InvalidPort,
            E::EmptyName => CoreError::EmptyName,
            E::NameTooLong { max } => CoreError::NameTooLong { max },
            E::NameTaken { project } => CoreError::NameTaken { project },
            E::NotExecutable { path } => CoreError::NotExecutable { path },
            E::InvalidForceStopAfter { choices } => CoreError::InvalidForceStopAfter { choices },
            E::InvalidLogLinesKept { choices } => CoreError::InvalidLogLinesKept { choices },
            E::NoRuns => CoreError::NoRuns,
            E::ExportFailed { path, reason } => CoreError::ExportFailed { path, reason },
            E::ProcessExited { pid } => CoreError::ProcessExited { pid },
            E::SignalFailed { pid, reason } => CoreError::SignalFailed { pid, reason },
            E::AgentServerSurvived { name } => CoreError::AgentServerSurvived { name },
        }
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn core_errors_keep_their_facts_and_english() {
        let error = CoreError::from(bunode_core::Error::Save {
            path: "/x/config.json".into(),
            source: std::io::Error::other("disk full"),
        });
        assert!(matches!(&error, CoreError::Save { path, reason }
            if path == "/x/config.json" && reason == "disk full"));
        assert_eq!(error.to_string(), "Couldn't save /x/config.json: disk full");
        let error = CoreError::from(bunode_core::Error::NameTaken {
            project: "acme".into(),
        });
        assert_eq!(error.to_string(), "Name already used in acme");
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
    /// Where diagnostics go, normally `~/Library/Logs/Bunode`; `None` for none.
    pub diagnostics_dir: Option<String>,
}

#[derive(uniffi::Object)]
pub struct BunodeCore {
    supervisor: bunode_core::Supervisor,
}

#[uniffi::export]
impl BunodeCore {
    #[uniffi::constructor]
    pub fn new(options: CoreOptions, listener: Box<dyn CoreListener>) -> Arc<Self> {
        let mut supervisor_options =
            bunode_core::SupervisorOptions::new(options.data_dir, options.runner_path);
        supervisor_options.diagnostics_dir = options.diagnostics_dir.map(PathBuf::from);
        let supervisor =
            bunode_core::Supervisor::new(supervisor_options, Arc::new(ListenerAdapter(listener)));
        Arc::new(BunodeCore { supervisor })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.supervisor.snapshot()
    }

    /// Registers a folder as a Project.
    pub fn add_project(&self, path: String) -> Result<AddProjectOutcome, CoreError> {
        Ok(self.supervisor.add_project(&PathBuf::from(path))?)
    }

    /// Settings › Projects › package manager; `None` is Auto.
    pub fn set_package_manager(
        &self,
        project_id: u64,
        manager: Option<PackageManager>,
    ) -> Result<(), CoreError> {
        Ok(self.supervisor.set_package_manager(project_id, manager)?)
    }

    /// Settings › Projects: the short name for a Project; blank clears it.
    pub fn set_project_alias(&self, project_id: u64, alias: String) -> Result<(), CoreError> {
        Ok(self.supervisor.set_project_alias(project_id, &alias)?)
    }

    /// Removes a Project, stopping its Runs first. Returns at once.
    pub fn remove_project(&self, project_id: u64) -> Result<(), CoreError> {
        Ok(self.supervisor.remove_project(project_id)?)
    }

    /// Points a Project whose folder moved at its new place.
    pub fn relocate_project(&self, project_id: u64, path: String) -> Result<(), CoreError> {
        Ok(self
            .supervisor
            .relocate_project(project_id, std::path::Path::new(&path))?)
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

    /// Saves the Service view's settings; a live Run gets them next time.
    pub fn update_service(&self, service_id: u64, edit: ServiceEdit) -> Result<(), CoreError> {
        Ok(self.supervisor.update_service(service_id, edit)?)
    }

    /// Deletes a Service, stopping its Run first. Returns at once.
    pub fn delete_service(&self, service_id: u64) -> Result<(), CoreError> {
        Ok(self.supervisor.delete_service(service_id)?)
    }

    /// Starts a Run unless one is live, or reports that its Port is taken.
    /// Returns at once.
    pub fn start(&self, service_id: u64) -> Result<StartOutcome, CoreError> {
        Ok(self.supervisor.start(service_id)?)
    }

    /// `Use :N`: starts this one Run with `PORT=N`.
    pub fn start_with_port(&self, service_id: u64, port: u16) -> Result<StartOutcome, CoreError> {
        Ok(self.supervisor.start_with_port(service_id, port)?)
    }

    /// Whether something listens on `port`.
    pub fn port_in_use(&self, port: u16) -> bool {
        self.supervisor.port_in_use(port)
    }

    /// Looks for other Node and Bun processes now.
    pub fn rescan(&self) {
        self.supervisor.rescan();
    }

    /// Other processes › Stop… › Send SIGTERM, if `pid` is still the
    /// process that started at `start`.
    pub fn stop_discovered(&self, pid: u32, start: u64) -> Result<(), CoreError> {
        Ok(self.supervisor.stop_discovered(pid, start)?)
    }

    /// Stops an Agent Server: SIGTERM to its group, SIGKILL after Force
    /// stop after. Blocks until it is gone, so call it off the main thread.
    pub fn stop_agent_server(&self, pid: u32) -> Result<(), CoreError> {
        Ok(self.supervisor.stop_agent_server(pid)?)
    }

    /// The Force Kill alert: SIGKILL to the process tree, each process
    /// checked first.
    pub fn force_kill_discovered(&self, pid: u32, start: u64) -> Result<(), CoreError> {
        Ok(self.supervisor.force_kill_discovered(pid, start)?)
    }

    /// Quit: stops every live Run. Returns how many there were.
    pub fn stop_all(&self) -> u32 {
        self.supervisor.stop_all() as u32
    }

    /// Stops the Run completely, then starts a new one. Returns at once.
    pub fn restart(&self, service_id: u64) -> Result<(), CoreError> {
        Ok(self.supervisor.restart(service_id)?)
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

    /// General › List.
    pub fn set_list_layout(&self, layout: ListLayout) -> Result<(), CoreError> {
        Ok(self.supervisor.set_list_layout(layout)?)
    }

    /// Projects › Show dev servers started by agents.
    pub fn set_show_agent_servers(&self, show: bool) -> Result<(), CoreError> {
        Ok(self.supervisor.set_show_agent_servers(show)?)
    }

    /// The popover opened: rereads the agent launch files that changed.
    pub fn check_launch_files(&self) {
        self.supervisor.check_launch_files();
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

    /// Logs › Export…: the full log of the current or last Run.
    pub fn export_log(&self, service_id: u64, destination: String) -> Result<(), CoreError> {
        Ok(self
            .supervisor
            .export_log(service_id, std::path::Path::new(&destination))?)
    }

    /// Logs › Open in Console: the newest file of the current or last Run.
    pub fn current_log_file(&self, service_id: u64) -> Option<String> {
        self.supervisor
            .current_log_file(service_id)
            .map(|p| p.display().to_string())
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
                diagnostics_dir: None,
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
