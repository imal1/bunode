//! The read-only view of the Supervisor's state that the UI renders.
//! The UI never derives state on its own; it shows the latest snapshot.

use crate::model::PackageManager;

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// In registration order.
    pub projects: Vec<ProjectState>,
    pub settings: SettingsState,
    /// Set when the saved configuration could not be read at launch.
    pub config_issue: Option<String>,
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
    pub command: String,
    /// Relative to the Project's directory; `None` is the directory itself.
    pub folder: Option<String>,
    pub port: Option<u16>,
    pub run_through_shell: bool,
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
