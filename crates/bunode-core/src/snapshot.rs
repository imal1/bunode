//! The read-only view of the Supervisor's state that the UI renders.
//! The UI never derives state on its own; it shows the latest snapshot.

use crate::model::PackageManager;

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// In registration order.
    pub projects: Vec<ProjectState>,
    pub settings: SettingsState,
    pub environment: EnvironmentState,
    /// Set when the saved configuration could not be read at launch.
    pub config_issue: Option<String>,
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
    pub problem: Option<String>,
    /// Node.js, then Bun.
    pub tools: Vec<ToolState>,
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
    pub name: String,
    pub path: String,
    /// The package manager in effect: the user's pick, else the detected one.
    pub package_manager: PackageManager,
    pub detected_package_manager: PackageManager,
    /// What the user picked in Settings › Projects; `None` is Auto.
    pub package_manager_choice: Option<PackageManager>,
    /// Lockfiles of several package managers were found and the user has
    /// not picked one yet.
    pub several_lockfiles: bool,
    /// In creation order.
    pub services: Vec<ServiceState>,
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
    /// How the last Run ended, for Crashed and Failed:
    /// "Exited with code 1", "Couldn't start …".
    pub last_exit: Option<String>,
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
    pub force_stop_after_secs: u32,
    pub log_lines_kept: u32,
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
