//! UniFFI bindings that expose `bunode-core` to the Swift app.
//!
//! This is the only crate that knows about UniFFI. It stays a thin adapter:
//! no process-management logic lives here. Core types that cross the
//! boundary unchanged are re-declared with `#[uniffi::remote]`, which must
//! list the same fields as the core definition.

use std::path::PathBuf;
use std::sync::Arc;

use bunode_core::{
    AddProjectOutcome, NewService, PackageManager, ProjectScripts, ProjectState, ScriptGroup,
    ScriptOption, ServiceState, SettingsState, Snapshot,
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
    pub config_issue: Option<String>,
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
            bunode_core::SupervisorOptions {
                data_dir: PathBuf::from(options.data_dir),
            },
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
