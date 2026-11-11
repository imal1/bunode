//! The read-only view of the Supervisor's state that the UI renders.
//! The UI never derives state on its own; it shows the latest snapshot.

use crate::model::{ListLayout, PackageManager};

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// In registration order.
    pub projects: Vec<ProjectState>,
    pub settings: SettingsState,
    pub environment: EnvironmentState,
    /// Set when the saved configuration could not be read at launch.
    pub config_issue: Option<ConfigIssue>,
    /// Node and Bun processes Bunode did not start, oldest first.
    pub discovered: Vec<crate::discovery::DiscoveredProcess>,
}

/// The environment new Runs get (Advanced › Shell environment).
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentState {
    /// The login shell it is read from.
    pub shell: String,
    pub variable_count: u32,
    /// When it was last read, in seconds since 1970; `None` until then.
    pub loaded_at: Option<i64>,
    pub is_loading: bool,
    /// Why the shell's environment could not be read; Runs then get
    /// Bunode's own environment.
    pub problem: Option<EnvironmentProblem>,
    /// Node.js, then Bun.
    pub tools: Vec<ToolState>,
}

/// Why the saved configuration could not be used. Bunode then starts with
/// an empty configuration and keeps the unreadable file next to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigIssue {
    /// The configuration file.
    pub path: String,
    /// Where the unreadable file was moved; `None` if it couldn't be.
    pub backup: Option<String>,
    pub problem: ConfigProblem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigProblem {
    /// It couldn't be read or parsed; `detail` is the system's or the
    /// parser's English message.
    Unreadable { detail: String },
    /// A newer Bunode wrote it, in this format version.
    NewerFormat { version: u32 },
}

/// Why the login shell's environment couldn't be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvironmentProblem {
    #[error("couldn't start {shell}: {reason}")]
    ShellFailed { shell: String, reason: String },
    #[error("{shell} printed no environment")]
    NoOutput { shell: String },
    #[error("{shell} took longer than {seconds} s")]
    TimedOut { shell: String, seconds: u32 },
    #[error("{reason}")]
    ReadFailed { reason: String },
}

/// How a Run ended or why it couldn't start. The app words it; `Display`
/// is the English line written into the Run's log.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunNote {
    #[error("Exited")]
    Exited,
    #[error("Exited with code {code}")]
    ExitedWithCode { code: i32 },
    /// `signal` is its name, such as "SIGKILL".
    #[error("Ended by {signal}")]
    EndedBySignal { signal: String },
    #[error("The command has an unmatched quote.")]
    UnmatchedQuote,
    #[error("The command is empty.")]
    EmptyCommand,
    /// The Run's folder is gone.
    #[error("{path} is missing.")]
    FolderMissing { path: String },
    /// A bare program name that no `PATH` folder has.
    #[error("{program} wasn't found in PATH.")]
    NotInPath { program: String },
    /// A program given by path that doesn't exist.
    #[error("{program} doesn't exist.")]
    ProgramMissing { program: String },
    #[error("Couldn't start {program}: {reason}")]
    CouldNotStart { program: String, reason: String },
    #[error("Couldn't start bunode-runner: {reason}")]
    RunnerFailed { reason: String },
    #[error("bunode-runner exited unexpectedly ({status}).")]
    RunnerExited { status: String },
    #[error("Lost track of bunode-runner: {reason}")]
    RunnerLost { reason: String },
    #[error("The Run ended for an unknown reason.")]
    UnknownEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Node,
    Bun,
}

impl Tool {
    pub fn executable(self) -> &'static str {
        match self {
            Tool::Node => "node",
            Tool::Bun => "bun",
        }
    }
}

/// Where Runs find `node` or `bun`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolState {
    pub tool: Tool,
    /// `None` when it was not found.
    pub path: Option<String>,
    pub version: Option<String>,
    /// The user picked this path in Settings instead of the one in PATH.
    pub is_custom: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectState {
    pub id: u64,
    /// What Bunode calls it: the alias, or else the folder's name.
    pub name: String,
    pub folder_name: String,
    pub path: String,
    /// The package manager in effect: the user's pick, else the detected one.
    pub package_manager: PackageManager,
    pub detected_package_manager: PackageManager,
    /// What the user picked in Settings › Projects; `None` is Auto.
    pub package_manager_choice: Option<PackageManager>,
    /// Lockfiles of several package managers were found and the user has
    /// not picked one yet.
    pub several_lockfiles: bool,
    /// The Project's folder is gone (moved or deleted).
    pub is_missing: bool,
    /// Discovered Processes running in the Project's folder or below it
    /// (their PIDs, in `Snapshot::discovered` order): Home shows them as
    /// read-only External rows. Never a Service's Run.
    pub external: Vec<u32>,
    /// In creation order.
    pub services: Vec<ServiceState>,
    /// Shown after the Services: in launch-file order, then oldest first.
    pub agent_servers: Vec<AgentServerState>,
}

/// A dev server an agent started from its launch settings in the
/// Project's root (an Agent Server, ADR-0007). Shown, never claimed.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentServerState {
    pub project_id: u64,
    /// The agent that started it: "Claude".
    pub agent: String,
    /// The launch configuration's `name`.
    pub name: String,
    /// The configuration's `runtimeExecutable` and `runtimeArgs`.
    pub command: String,
    /// The agent's launcher, which leads the process group.
    pub pid: u32,
    pub pgid: u32,
    /// The TCP ports any process of the group listens on, the
    /// configuration's `port` first.
    pub live_ports: Vec<u16>,
    /// Bunode is ending its process group.
    pub stopping: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceState {
    pub id: u64,
    pub project_id: u64,
    pub name: String,
    /// The name as Home shows it: "web › dev" for a Service in a subfolder.
    pub display_name: String,
    pub command: String,
    /// Relative to the Project's directory; `None` is the directory itself.
    pub folder: Option<String>,
    pub port: Option<u16>,
    pub run_through_shell: bool,
    pub status: RunStatus,
    /// The current Run's number, else the last one's; `None` before the
    /// first Run.
    pub run_number: Option<u64>,
    /// The Run's command process, which leads its process group.
    pub pid: Option<u32>,
    /// How the last Run ended, for Crashed and Failed.
    pub last_exit: Option<RunNote>,
    /// The log of the current or last Run: `seq` of the first line shown,
    /// how many are shown, and how many were dropped (`LogInfo`).
    pub log_first: u64,
    pub log_count: u64,
    pub log_dropped: u64,
    /// With a Logs search: the first match number and how many matches,
    /// and a number that changes whenever the search does.
    pub log_match_first: u64,
    pub log_match_count: u64,
    pub log_filter_generation: u64,
    /// The settings changed while the Run was live; they apply to the next
    /// Run ("Saved. Applies on the next Run.").
    pub has_pending_config: bool,
    /// How the last Run ended; `None` while it is live or before any.
    pub ended_by: Option<RunEnd>,
    /// The TCP ports the live Run listens on, any process of its group.
    pub live_ports: Vec<u16>,
}

/// Why a Run ended (requirements §3.8): Bunode tells a Stop by the user
/// apart from a crash and from Bunode quitting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEnd {
    StoppedByUser,
    BunodeQuit,
    /// Exited on its own without an error.
    Exited,
    Crashed,
    FailedToStart,
}

/// Where a Service's Run is in its life (design-handoff §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// No Run, or the last one ended normally or was stopped.
    Stopped,
    Starting,
    Running,
    Stopping,
    Restarting,
    /// The last Run exited on its own with an error.
    Crashed,
    /// The last Run could not start.
    Failed,
}

impl RunStatus {
    /// A Run exists and may still own processes.
    pub fn is_live(self) -> bool {
        matches!(
            self,
            RunStatus::Starting | RunStatus::Running | RunStatus::Stopping | RunStatus::Restarting
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsState {
    pub show_running_count: bool,
    pub list_layout: ListLayout,
    pub show_agent_servers: bool,
    pub force_stop_after_secs: u32,
    pub log_lines_kept: u32,
}

/// What Start did (design-handoff §5.3).
#[derive(Debug, Clone, PartialEq)]
pub enum StartOutcome {
    /// A Run is starting, or was already live.
    Started,
    /// The Service's port is taken; nothing started.
    PortInUse {
        port: u16,
        /// Who holds it, when it is one of the user's processes.
        owner: Option<PortConflictOwner>,
        /// The first free port after it, for `Use :N`.
        suggestion: Option<u16>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortConflictOwner {
    pub pid: u32,
    pub command: String,
    /// The app it was started from; `None` when unknown.
    pub source: Option<String>,
    /// The owner is another of Bunode's Services.
    pub service_id: Option<u64>,
    /// The owner is an Agent Server; `command` is its name.
    pub agent_server: bool,
}

/// The result of adding a Project folder.
#[derive(Debug, Clone, PartialEq)]
pub enum AddProjectOutcome {
    Added {
        project: ProjectState,
    },
    /// The folder is already registered; nothing changed.
    AlreadyAdded {
        project: ProjectState,
    },
}

/// The scripts the New Service view offers for a Project.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectScripts {
    /// `false` when the Project has no `package.json`: the view then hides
    /// the scripts and asks for a command.
    pub has_package_json: bool,
    /// The Project's own scripts first; workspace packages follow.
    pub groups: Vec<ScriptGroup>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptGroup {
    /// The workspace package's name; `None` for the Project's root.
    pub package_name: Option<String>,
    /// Where its scripts run, relative to the Project; `None` for the root.
    pub folder: Option<String>,
    pub scripts: Vec<ScriptOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptOption {
    pub name: String,
    /// The command that runs it with the Project's package manager.
    pub command: String,
    /// A Service with this command and folder already exists.
    pub used: bool,
}

/// The Service view's settings, saved as a whole.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceEdit {
    pub command: String,
    /// Relative to the Project's directory; `None` is the directory itself.
    pub folder: Option<String>,
    pub port: Option<u16>,
    pub run_through_shell: bool,
}

/// What the user fills in on the New Service view.
#[derive(Debug, Clone, PartialEq)]
pub struct NewService {
    pub name: String,
    pub command: String,
    /// Relative to the Project's directory; `None` is the directory itself.
    pub folder: Option<String>,
}
