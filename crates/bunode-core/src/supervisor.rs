//! The Supervisor owns Bunode's true state: the configuration and, later,
//! every Run. The UI sends it commands and renders its snapshots.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::model::{Config, Project, ProjectId};
use crate::package::{self, Detection};
use crate::snapshot::{AddProjectOutcome, ProjectState, ServiceState, SettingsState, Snapshot};
use crate::store::ConfigStore;

/// Changes are reported at most this often (requirements §2.2).
const COALESCE: Duration = Duration::from_millis(100);

/// Told when the state changed. It then asks for a `snapshot()`. Calls come
/// from a background thread, at most once per 100 ms.
pub trait Listener: Send + Sync {
    fn state_changed(&self);
}

pub struct SupervisorOptions {
    /// Where `config.json` lives, normally `~/Library/Application Support/Bunode`.
    pub data_dir: PathBuf,
}

pub struct Supervisor {
    store: ConfigStore,
    state: Mutex<State>,
    notifier: Notifier,
}

struct State {
    config: Config,
    detections: HashMap<ProjectId, Detection>,
    config_issue: Option<String>,
}

impl Supervisor {
    pub fn new(options: SupervisorOptions, listener: Arc<dyn Listener>) -> Supervisor {
        let store = ConfigStore::new(&options.data_dir);
        let (config, issue) = store.load();
        let detections = config
            .projects
            .iter()
            .map(|p| (p.id, package::detect(&p.path)))
            .collect();
        Supervisor {
            store,
            state: Mutex::new(State {
                config,
                detections,
                config_issue: issue.map(|i| i.message),
            }),
            notifier: Notifier::new(listener),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let state = self.lock();
        Snapshot {
            projects: state
                .config
                .projects
                .iter()
                .map(|p| project_state(p, &state.detections))
                .collect(),
            settings: SettingsState {
                show_running_count: state.config.settings.show_running_count,
                force_stop_after_secs: state.config.settings.force_stop_after_secs,
                log_lines_kept: state.config.settings.log_lines_kept,
            },
            config_issue: state.config_issue.clone(),
        }
    }

    /// Registers `path` as a Project and detects its package manager.
    pub fn add_project(&self, path: &Path) -> Result<AddProjectOutcome> {
        let not_a_folder = || Error::NotAFolder {
            path: path.display().to_string(),
        };
        let path = fs::canonicalize(path).map_err(|_| not_a_folder())?;
        if !path.is_dir() {
            return Err(not_a_folder());
        }
        let detection = package::detect(&path);

        let mut state = self.lock();
        if let Some(existing) = state.config.projects.iter().find(|p| p.path == path) {
            return Ok(AddProjectOutcome::AlreadyAdded {
                project: project_state(existing, &state.detections),
            });
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let mut config = state.config.clone();
        let project = Project {
            id: config.allocate_id(),
            name,
            path,
            package_manager: None,
            services: Vec::new(),
        };
        config.projects.push(project.clone());
        self.save(&config)?;

        state.config = config;
        state.detections.insert(project.id, detection);
        let added = project_state(&project, &state.detections);
        drop(state);
        self.notifier.notify();
        Ok(AddProjectOutcome::Added { project: added })
    }

    fn save(&self, config: &Config) -> Result<()> {
        self.store.save(config).map_err(|source| Error::Save {
            path: self.store.path().display().to_string(),
            source,
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock must not take the whole app down.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn project_state(project: &Project, detections: &HashMap<ProjectId, Detection>) -> ProjectState {
    let detection = detections
        .get(&project.id)
        .cloned()
        .unwrap_or_else(|| package::detect(&project.path));
    ProjectState {
        id: project.id,
        name: project.name.clone(),
        path: project.path.display().to_string(),
        package_manager: project.package_manager.unwrap_or(detection.manager),
        detected_package_manager: detection.manager,
        package_manager_choice: project.package_manager,
        several_lockfiles: detection.several_lockfiles && project.package_manager.is_none(),
        services: project
            .services
            .iter()
            .map(|s| ServiceState {
                id: s.id,
                project_id: project.id,
                name: s.name.clone(),
                command: s.command.clone(),
                folder: s.folder.as_ref().map(|f| f.display().to_string()),
                port: s.port,
                run_through_shell: s.run_through_shell,
            })
            .collect(),
    }
}

/// Calls the listener on its own thread, coalescing bursts of changes.
struct Notifier {
    tx: mpsc::Sender<()>,
}

impl Notifier {
    fn new(listener: Arc<dyn Listener>) -> Notifier {
        let (tx, rx) = mpsc::channel::<()>();
        thread::Builder::new()
            .name("bunode-notify".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    while rx.try_recv().is_ok() {}
                    listener.state_changed();
                    thread::sleep(COALESCE);
                }
            })
            .expect("spawn notifier thread");
        Notifier { tx }
    }

    fn notify(&self) {
        let _ = self.tx.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PackageManager;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Counter(AtomicUsize);
    impl Listener for Counter {
        fn state_changed(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn open(data: &Path) -> (Supervisor, Arc<Counter>) {
        let counter = Arc::new(Counter::default());
        let supervisor = Supervisor::new(
            SupervisorOptions {
                data_dir: data.to_path_buf(),
            },
            counter.clone(),
        );
        (supervisor, counter)
    }

    #[test]
    fn adds_a_project_and_keeps_it_across_restarts() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        let api = code.path().join("api");
        fs::create_dir(&api).unwrap();
        fs::write(api.join("bun.lock"), "").unwrap();

        let (supervisor, counter) = open(data.path());
        let outcome = supervisor.add_project(&api).unwrap();
        let AddProjectOutcome::Added { project } = outcome else {
            panic!("expected Added, got {outcome:?}");
        };
        assert_eq!(project.name, "api");
        assert_eq!(project.package_manager, PackageManager::Bun);
        assert!(!project.several_lockfiles);

        // The listener hears about it.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while counter.0.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(counter.0.load(Ordering::SeqCst) >= 1);
        drop(supervisor);

        let (reloaded, _) = open(data.path());
        let snapshot = reloaded.snapshot();
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.projects[0].name, "api");
        assert_eq!(snapshot.projects[0].package_manager, PackageManager::Bun);
        assert!(snapshot.config_issue.is_none());
    }

    #[test]
    fn adding_the_same_folder_twice_is_reported() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        let (supervisor, _) = open(data.path());
        supervisor.add_project(code.path()).unwrap();
        // Through a different spelling of the same path.
        let again = code.path().join(".");
        assert!(matches!(
            supervisor.add_project(&again).unwrap(),
            AddProjectOutcome::AlreadyAdded { .. }
        ));
        assert_eq!(supervisor.snapshot().projects.len(), 1);
    }

    #[test]
    fn rejects_files_and_missing_paths() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        let file = code.path().join("package.json");
        fs::write(&file, "{}").unwrap();
        let (supervisor, _) = open(data.path());
        assert!(matches!(
            supervisor.add_project(&file),
            Err(Error::NotAFolder { .. })
        ));
        assert!(matches!(
            supervisor.add_project(&code.path().join("missing")),
            Err(Error::NotAFolder { .. })
        ));
    }

    #[test]
    fn several_lockfiles_show_until_a_package_manager_is_picked() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        fs::write(code.path().join("yarn.lock"), "").unwrap();
        fs::write(code.path().join("package-lock.json"), "").unwrap();
        let (supervisor, _) = open(data.path());
        let AddProjectOutcome::Added { project } = supervisor.add_project(code.path()).unwrap()
        else {
            panic!()
        };
        assert!(project.several_lockfiles);
        assert_eq!(project.package_manager, PackageManager::Yarn);
    }

    #[test]
    fn corrupt_config_starts_empty_with_an_issue() {
        let data = tempfile::tempdir().unwrap();
        fs::write(data.path().join("config.json"), "not json").unwrap();
        let (supervisor, _) = open(data.path());
        let snapshot = supervisor.snapshot();
        assert!(snapshot.projects.is_empty());
        assert!(snapshot.config_issue.unwrap().contains("couldn't be read"));
    }
}
