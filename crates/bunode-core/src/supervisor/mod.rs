//! The Supervisor owns Bunode's true state: the configuration and every
//! Run. The UI sends it commands, which return at once, and renders its
//! snapshots; it is told about changes through a single coalesced listener.

mod run;

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::Duration;

use tokio::sync::mpsc as async_mpsc;

use crate::error::{Error, Result};
use crate::launch;
use crate::model::{Config, PackageManager, Project, ProjectId, Service, ServiceId};
use crate::package::{self, Detection};
use crate::snapshot::{
    AddProjectOutcome, NewService, ProjectScripts, ProjectState, RunStatus, ScriptGroup,
    ScriptOption, ServiceState, SettingsState, Snapshot,
};
use crate::store::ConfigStore;

use run::{Control, Outcome};

/// Changes are reported at most this often (requirements §2.2).
const COALESCE: Duration = Duration::from_millis(100);

/// Service names are 1–40 characters (design-handoff §6.2).
pub const MAX_SERVICE_NAME_LENGTH: usize = 40;

/// Told when the state changed. It then asks for a `snapshot()`. Calls come
/// from a background thread, at most once per 100 ms.
pub trait Listener: Send + Sync {
    fn state_changed(&self);
}

pub struct SupervisorOptions {
    /// Where `config.json` lives, normally `~/Library/Application Support/Bunode`.
    pub data_dir: PathBuf,
    /// The `bunode-runner` executable, normally inside the app bundle.
    pub runner_path: PathBuf,
}

pub struct Supervisor {
    shared: Arc<Shared>,
    runtime: tokio::runtime::Runtime,
}

/// What the Supervisor's background tasks share with it.
pub(crate) struct Shared {
    store: ConfigStore,
    state: Mutex<State>,
    notifier: Notifier,
    runner_path: PathBuf,
    /// Held while creating a lifeline and spawning a runner, so that no
    /// other spawn can inherit the lifeline before it is close-on-exec.
    spawn_lock: Mutex<()>,
}

struct State {
    config: Config,
    detections: HashMap<ProjectId, Detection>,
    config_issue: Option<String>,
    /// The current or last Run of each Service that has had one.
    runs: HashMap<ServiceId, RunEntry>,
    /// The environment new Runs start from.
    base_env: Vec<(OsString, OsString)>,
}

struct RunEntry {
    run_number: u64,
    status: RunStatus,
    pid: Option<u32>,
    stop_requested: bool,
    /// How to reach the Run's task while it is live.
    control: Option<async_mpsc::UnboundedSender<Control>>,
    last_exit: Option<String>,
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
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("bunode-supervisor")
            .enable_all()
            .build()
            .expect("start the Supervisor's runtime");
        Supervisor {
            shared: Arc::new(Shared {
                store,
                state: Mutex::new(State {
                    config,
                    detections,
                    config_issue: issue.map(|i| i.message),
                    runs: HashMap::new(),
                    base_env: std::env::vars_os().collect(),
                }),
                notifier: Notifier::new(listener),
                runner_path: options.runner_path,
                spawn_lock: Mutex::new(()),
            }),
            runtime,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let state = self.shared.lock();
        Snapshot {
            projects: state
                .config
                .projects
                .iter()
                .map(|p| project_state(p, &state))
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

        let mut state = self.shared.lock();
        if let Some(existing) = state.config.projects.iter().find(|p| p.path == path) {
            return Ok(AddProjectOutcome::AlreadyAdded {
                project: project_state(existing, &state),
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
        self.shared.save(&config)?;

        state.config = config;
        state.detections.insert(project.id, detection);
        let added = project_state(&project, &state);
        drop(state);
        self.shared.notifier.notify();
        Ok(AddProjectOutcome::Added { project: added })
    }

    /// The scripts the New Service view offers, read fresh from disk.
    pub fn project_scripts(&self, project_id: ProjectId) -> Result<ProjectScripts> {
        let (path, manager, services) = {
            let state = self.shared.lock();
            let project = state
                .config
                .project(project_id)
                .ok_or(Error::UnknownProject)?;
            (
                project.path.clone(),
                effective_manager(project, &state.detections),
                project.services.clone(),
            )
        };
        let scripts = package::scripts(&path);
        let used = |command: &str, folder: Option<&Path>| {
            services
                .iter()
                .any(|s| s.command == command && s.folder.as_deref() == folder)
        };
        let root = ScriptGroup {
            package_name: None,
            folder: None,
            scripts: scripts
                .iter()
                .flatten()
                .map(|script| {
                    let command = package::script_command(manager, &script.name);
                    ScriptOption {
                        used: used(&command, None),
                        name: script.name.clone(),
                        command,
                    }
                })
                .collect(),
        };
        Ok(ProjectScripts {
            has_package_json: scripts.is_some(),
            groups: vec![root],
        })
    }

    /// Adds a Service to a Project. Nothing is started.
    pub fn add_service(&self, project_id: ProjectId, new: NewService) -> Result<ServiceState> {
        let mut state = self.shared.lock();
        let project = state
            .config
            .project(project_id)
            .ok_or(Error::UnknownProject)?;
        let name = validate_service_name(project, &new.name, None)?;
        let command = new.command.trim();
        if command.is_empty() {
            return Err(Error::Invalid("Enter a command.".into()));
        }
        let folder = new
            .folder
            .map(|f| f.trim().to_owned())
            .filter(|f| !f.is_empty() && f != ".")
            .map(PathBuf::from);

        let mut config = state.config.clone();
        let service = Service {
            id: config.allocate_id(),
            name,
            command: command.to_owned(),
            folder,
            port: None,
            run_through_shell: false,
            env: Default::default(),
            runs: 0,
        };
        let project = config
            .project_mut(project_id)
            .ok_or(Error::UnknownProject)?;
        project.services.push(service.clone());
        let added = service_state(project, &service, None);
        self.shared.save(&config)?;
        state.config = config;
        drop(state);
        self.shared.notifier.notify();
        Ok(added)
    }

    /// General › Show running count.
    pub fn set_show_running_count(&self, show: bool) -> Result<()> {
        self.update_settings(|settings| settings.show_running_count = show)
    }

    fn update_settings(&self, change: impl FnOnce(&mut crate::model::Settings)) -> Result<()> {
        let mut state = self.shared.lock();
        let mut config = state.config.clone();
        change(&mut config.settings);
        if config.settings == state.config.settings {
            return Ok(());
        }
        self.shared.save(&config)?;
        state.config = config;
        drop(state);
        self.shared.notifier.notify();
        Ok(())
    }

    // MARK: - Runs

    /// Starts a Run of the Service, unless one is already live. Returns at
    /// once; the Run's progress arrives through snapshots.
    pub fn start(&self, service_id: ServiceId) -> Result<()> {
        let mut state = self.shared.lock();
        if state
            .runs
            .get(&service_id)
            .is_some_and(|run| run.status.is_live())
        {
            return Ok(());
        }
        let (project, service) = find_service(&state.config, service_id)?;
        let spec = launch::launch_spec(project, service, &state.config.settings, &state.base_env);
        let project_id = project.id;

        // Run numbers count every Run, including ones that fail to start.
        let mut config = state.config.clone();
        let run_number = {
            let service = config
                .project_mut(project_id)
                .and_then(|p| p.services.iter_mut().find(|s| s.id == service_id))
                .ok_or(Error::UnknownService)?;
            service.runs += 1;
            service.runs
        };
        // A failed save must not keep the Service from starting.
        let _ = self.shared.store.save(&config);
        state.config = config;

        let spec = match spec {
            Ok(spec) => spec,
            Err(message) => {
                state.runs.insert(
                    service_id,
                    RunEntry {
                        run_number,
                        status: RunStatus::Failed,
                        pid: None,
                        stop_requested: false,
                        control: None,
                        last_exit: Some(message),
                    },
                );
                drop(state);
                self.shared.notifier.notify();
                return Ok(());
            }
        };

        let (control, control_rx) = async_mpsc::unbounded_channel();
        state.runs.insert(
            service_id,
            RunEntry {
                run_number,
                status: RunStatus::Starting,
                pid: None,
                stop_requested: false,
                control: Some(control),
                last_exit: None,
            },
        );
        drop(state);
        self.shared.notifier.notify();

        let shared = self.shared.clone();
        self.runtime.spawn(async move {
            let outcome = run::drive(&shared, service_id, run_number, spec, control_rx).await;
            shared.finish(service_id, run_number, outcome);
        });
        Ok(())
    }

    /// Stops the Service's Run: SIGTERM to its process group, SIGKILL after
    /// the Force stop timeout. Returns at once.
    pub fn stop(&self, service_id: ServiceId) {
        self.control(service_id, Control::Stop);
    }

    /// Force-kills the Service's Run (SIGKILL to the whole group).
    pub fn force_kill(&self, service_id: ServiceId) {
        self.control(service_id, Control::Kill);
    }

    fn control(&self, service_id: ServiceId, control: Control) {
        let mut state = self.shared.lock();
        let Some(run) = state.runs.get_mut(&service_id) else {
            return;
        };
        let Some(sender) = run.control.as_ref() else {
            return;
        };
        let _ = sender.send(control);
        run.stop_requested = true;
        if matches!(run.status, RunStatus::Starting | RunStatus::Running) {
            run.status = RunStatus::Stopping;
        }
        drop(state);
        self.shared.notifier.notify();
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        // Closing every lifeline makes each runner end its group.
        let mut state = self.shared.lock();
        for run in state.runs.values_mut() {
            run.control = None;
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock must not take the whole app down.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save(&self, config: &Config) -> Result<()> {
        self.store.save(config).map_err(|source| Error::Save {
            path: self.store.path().display().to_string(),
            source,
        })
    }

    /// The runner reported that the command started.
    fn started(&self, service_id: ServiceId, run_number: u64, pid: u32) {
        let mut state = self.lock();
        let Some(run) = state.runs.get_mut(&service_id) else {
            return;
        };
        if run.run_number != run_number {
            return;
        }
        run.pid = Some(pid);
        if run.status == RunStatus::Starting {
            run.status = RunStatus::Running;
        }
        drop(state);
        self.notifier.notify();
    }

    /// The Run is over; record how it ended.
    fn finish(&self, service_id: ServiceId, run_number: u64, outcome: Outcome) {
        let mut state = self.lock();
        let Some(run) = state.runs.get_mut(&service_id) else {
            return;
        };
        if run.run_number != run_number {
            return;
        }
        let (status, note) = outcome.classify(run.stop_requested);
        run.status = status;
        run.last_exit = note;
        run.pid = None;
        run.control = None;
        drop(state);
        self.notifier.notify();
    }
}

fn find_service(config: &Config, service_id: ServiceId) -> Result<(&Project, &Service)> {
    config
        .projects
        .iter()
        .find_map(|p| {
            p.services
                .iter()
                .find(|s| s.id == service_id)
                .map(|s| (p, s))
        })
        .ok_or(Error::UnknownService)
}

/// Checks a Service name: 1–40 characters and unique in the Project,
/// ignoring case. Returns it trimmed.
fn validate_service_name(
    project: &Project,
    name: &str,
    except: Option<ServiceId>,
) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("Enter a name.".into()));
    }
    if name.chars().count() > MAX_SERVICE_NAME_LENGTH {
        return Err(Error::Invalid(format!(
            "Use at most {MAX_SERVICE_NAME_LENGTH} characters."
        )));
    }
    let taken = project
        .services
        .iter()
        .any(|s| Some(s.id) != except && s.name.to_lowercase() == name.to_lowercase());
    if taken {
        return Err(Error::Invalid(format!(
            "Name already used in {}",
            project.name
        )));
    }
    Ok(name.to_owned())
}

fn detection_of(project: &Project, detections: &HashMap<ProjectId, Detection>) -> Detection {
    detections
        .get(&project.id)
        .cloned()
        .unwrap_or_else(|| package::detect(&project.path))
}

fn effective_manager(
    project: &Project,
    detections: &HashMap<ProjectId, Detection>,
) -> PackageManager {
    project
        .package_manager
        .unwrap_or_else(|| detection_of(project, detections).manager)
}

fn service_state(project: &Project, service: &Service, run: Option<&RunEntry>) -> ServiceState {
    ServiceState {
        id: service.id,
        project_id: project.id,
        name: service.name.clone(),
        command: service.command.clone(),
        folder: service.folder.as_ref().map(|f| f.display().to_string()),
        port: service.port,
        run_through_shell: service.run_through_shell,
        status: run.map_or(RunStatus::Stopped, |r| r.status),
        run_number: run.map(|r| r.run_number),
        pid: run.and_then(|r| r.pid),
        last_exit: run.and_then(|r| r.last_exit.clone()),
    }
}

fn project_state(project: &Project, state: &State) -> ProjectState {
    let detection = detection_of(project, &state.detections);
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
            .map(|s| service_state(project, s, state.runs.get(&s.id)))
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
                runner_path: "/nonexistent/bunode-runner".into(),
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

    fn added_project(supervisor: &Supervisor, dir: &Path) -> ProjectState {
        match supervisor.add_project(dir).unwrap() {
            AddProjectOutcome::Added { project } => project,
            other => panic!("expected Added, got {other:?}"),
        }
    }

    fn new_service(name: &str, command: &str) -> NewService {
        NewService {
            name: name.into(),
            command: command.into(),
            folder: None,
        }
    }

    #[test]
    fn offers_scripts_with_package_manager_commands() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        fs::write(
            code.path().join("package.json"),
            r#"{"scripts": {"dev": "vite", "storybook": "storybook dev"}}"#,
        )
        .unwrap();
        fs::write(code.path().join("pnpm-lock.yaml"), "").unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, code.path());

        supervisor
            .add_service(project.id, new_service("dev", "pnpm dev"))
            .unwrap();
        let scripts = supervisor.project_scripts(project.id).unwrap();
        assert!(scripts.has_package_json);
        let options = &scripts.groups[0].scripts;
        assert_eq!(options.len(), 2);
        assert_eq!(
            (options[0].command.as_str(), options[0].used),
            ("pnpm dev", true)
        );
        assert_eq!(
            (options[1].command.as_str(), options[1].used),
            ("pnpm storybook", false)
        );
    }

    #[test]
    fn project_without_package_json_offers_no_scripts() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, code.path());
        let scripts = supervisor.project_scripts(project.id).unwrap();
        assert!(!scripts.has_package_json);
        assert!(scripts.groups[0].scripts.is_empty());
    }

    #[test]
    fn adds_services_in_order_and_keeps_them() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, code.path());
        supervisor
            .add_service(project.id, new_service(" dev ", " bun run dev "))
            .unwrap();
        supervisor
            .add_service(project.id, new_service("worker", "node worker.js"))
            .unwrap();
        drop(supervisor);

        let (reloaded, _) = open(data.path());
        let services = &reloaded.snapshot().projects[0].services;
        let names: Vec<&str> = services.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["dev", "worker"]);
        assert_eq!(services[0].command, "bun run dev");
        assert_eq!(services[0].port, None);
    }

    #[test]
    fn service_names_are_checked() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, code.path());
        supervisor
            .add_service(project.id, new_service("dev", "bun run dev"))
            .unwrap();

        let error = supervisor
            .add_service(project.id, new_service("DEV", "bun run dev"))
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("Name already used in {}", project.name)
        );
        assert!(
            supervisor
                .add_service(project.id, new_service("", "x"))
                .is_err()
        );
        assert!(
            supervisor
                .add_service(project.id, new_service(&"x".repeat(41), "x"))
                .is_err()
        );
        assert!(
            supervisor
                .add_service(project.id, new_service(&"x".repeat(40), "x"))
                .is_ok()
        );
        assert!(
            supervisor
                .add_service(project.id, new_service("ok", "  "))
                .is_err()
        );
        assert!(matches!(
            supervisor.add_service(9999, new_service("a", "b")),
            Err(Error::UnknownProject)
        ));
    }

    #[test]
    fn settings_persist() {
        let data = tempfile::tempdir().unwrap();
        let (supervisor, _) = open(data.path());
        assert!(supervisor.snapshot().settings.show_running_count);
        supervisor.set_show_running_count(false).unwrap();
        drop(supervisor);
        let (reloaded, _) = open(data.path());
        assert!(!reloaded.snapshot().settings.show_running_count);
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
