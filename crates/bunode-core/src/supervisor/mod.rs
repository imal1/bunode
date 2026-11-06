//! The Supervisor owns Bunode's true state: the configuration and every
//! Run. The UI sends it commands, which return at once, and renders its
//! snapshots; it is told about changes through a single coalesced listener.

mod run;

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc as async_mpsc;

use crate::diagnostics::Diagnostics;
use crate::discovery::{DiscoveredProcess, Ours, Scanner};
use crate::env;
use crate::error::{Error, Result};
use crate::launch;
use crate::leftovers::{self, RunRecord, RunRecords};
use crate::logfile::{Limits, LogFiles, RunLogWriter};
use crate::logs::{LogBuffer, LogLine, LogStream};
use crate::model::{Config, PackageManager, Project, ProjectId, Service, ServiceId};
use crate::package::{self, Detection};
use crate::snapshot::{
    AddProjectOutcome, EnvironmentState, NewService, ProjectScripts, ProjectState, RunEnd,
    RunStatus, ScriptGroup, ScriptOption, ServiceEdit, ServiceState, SettingsState, Snapshot, Tool,
    ToolState,
};
use crate::store::ConfigStore;

use run::{Control, Outcome};

/// Changes are reported at most this often (requirements §2.2).
const COALESCE: Duration = Duration::from_millis(100);

/// How often other Node and Bun processes are looked for.
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(3);

/// The choices of Advanced › Log lines kept.
pub const LOG_LINES_CHOICES: [u32; 3] = [5_000, 10_000, 50_000];

/// The choices of Advanced › Force stop after, in seconds.
pub const FORCE_STOP_CHOICES: [u32; 4] = [3, 5, 10, 30];

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
    /// The login shell whose environment Runs get; `$SHELL` by default.
    /// `None` keeps Bunode's own environment (tests).
    pub login_shell: Option<PathBuf>,
    /// How long the login shell gets to print its environment.
    pub environment_timeout: Duration,
    /// Where Run logs are written; `<data_dir>/Logs` by default.
    pub logs_dir: PathBuf,
    pub log_limits: Limits,
    /// Where the Supervisor and the runners write their diagnostics,
    /// normally `~/Library/Logs/Bunode`; `None` writes none.
    pub diagnostics_dir: Option<PathBuf>,
}

impl SupervisorOptions {
    pub fn new(data_dir: impl Into<PathBuf>, runner_path: impl Into<PathBuf>) -> SupervisorOptions {
        let data_dir = data_dir.into();
        SupervisorOptions {
            logs_dir: data_dir.join("Logs"),
            data_dir,
            runner_path: runner_path.into(),
            login_shell: Some(env::default_shell()),
            environment_timeout: Duration::from_secs(10),
            log_limits: Limits::default(),
            diagnostics_dir: None,
        }
    }
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
    login_shell: Option<PathBuf>,
    environment_timeout: Duration,
    log_files: LogFiles,
    /// The processes of every live Run, on disk for the next launch.
    records: Mutex<RunRecords>,
    diagnostics: Diagnostics,
    /// The runners' diagnostics file.
    runner_diagnostics: Option<PathBuf>,
    /// Where Runs' tasks run.
    runtime: tokio::runtime::Handle,
    /// Wakes the discovery scan before its interval is up.
    rescan: tokio::sync::Notify,
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
    environment: EnvironmentInfo,
    /// The Logs search of each Service; a new Run keeps it.
    log_filters: HashMap<ServiceId, String>,
    /// Projects to remove once their last live Run has ended.
    removing_projects: std::collections::HashSet<ProjectId>,
    /// Node and Bun processes Bunode did not start, from the last scan.
    discovered: Vec<DiscoveredProcess>,
    /// Discovered Processes sent SIGTERM, by PID and start time.
    exiting: std::collections::HashSet<(u32, u64)>,
}

struct EnvironmentInfo {
    loaded_at: Option<SystemTime>,
    loading: bool,
    problem: Option<String>,
    tools: Vec<ToolState>,
}

struct RunEntry {
    run_number: u64,
    status: RunStatus,
    pid: Option<u32>,
    stop_requested: bool,
    /// How to reach the Run's task while it is live.
    control: Option<async_mpsc::UnboundedSender<Control>>,
    last_exit: Option<String>,
    /// The Run's output in memory; kept after it ends, until the next Run.
    logs: Arc<Mutex<LogBuffer>>,
    /// The Run's log file. Kept after the Run ends, so an export can wait
    /// for the last lines to reach the disk; the next Run replaces it.
    file: Option<RunLogWriter>,
    /// Restart: start a new Run as soon as this one is over.
    restart_after: bool,
    /// Delete the Service as soon as this Run is over.
    delete_after: bool,
    /// The Service's settings changed while this Run was live.
    config_changed: bool,
    /// Stopped because Bunode is quitting.
    quitting: bool,
    ended_by: Option<RunEnd>,
}

/// Where a Run's output goes: memory for the Logs view, and its file.
#[derive(Clone)]
pub(crate) struct RunLog {
    pub(crate) buffer: Arc<Mutex<LogBuffer>>,
    pub(crate) file: RunLogWriter,
}

impl RunLog {
    pub(crate) fn line(&self, stream: LogStream, line: crate::ansi::StyledLine) {
        let time = now_ms();
        self.file.line(time, stream, &line.text);
        lock_logs(&self.buffer).push(stream, time, line);
    }

    pub(crate) fn system(&self, text: &str) {
        let time = now_ms();
        self.file.line(time, LogStream::System, text);
        lock_logs(&self.buffer).push_system(time, text);
    }
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn lock_logs(logs: &Mutex<LogBuffer>) -> MutexGuard<'_, LogBuffer> {
    logs.lock().unwrap_or_else(|e| e.into_inner())
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
        let diagnostics = Diagnostics::open(options.diagnostics_dir.as_deref(), "supervisor.log");
        diagnostics.log("Supervisor started");
        // Whatever the previous Bunode left running is ended in the
        // background; Runs started from now on get a fresh record file.
        let records = RunRecords::new(&options.data_dir);
        let leftover = records.load();
        records.save();
        let supervisor = Supervisor {
            shared: Arc::new(Shared {
                store,
                state: Mutex::new(State {
                    config,
                    detections,
                    config_issue: issue.map(|i| i.message),
                    runs: HashMap::new(),
                    // Until the login shell answers, Runs get Bunode's own.
                    base_env: std::env::vars_os().collect(),
                    environment: EnvironmentInfo {
                        loaded_at: None,
                        loading: false,
                        problem: None,
                        tools: Vec::new(),
                    },
                    log_filters: HashMap::new(),
                    removing_projects: Default::default(),
                    discovered: Vec::new(),
                    exiting: Default::default(),
                }),
                notifier: Notifier::new(listener),
                runner_path: options.runner_path,
                login_shell: options.login_shell,
                environment_timeout: options.environment_timeout,
                log_files: LogFiles::new(options.logs_dir, options.log_limits),
                records: Mutex::new(records),
                diagnostics,
                runner_diagnostics: options.diagnostics_dir.map(|d| d.join("runner.log")),
                runtime: runtime.handle().clone(),
                rescan: tokio::sync::Notify::new(),
                spawn_lock: Mutex::new(()),
            }),
            runtime,
        };
        supervisor.reload_environment();
        let shared = supervisor.shared.clone();
        supervisor.runtime.spawn(discover(shared));
        if !leftover.is_empty() {
            let shared = supervisor.shared.clone();
            supervisor.runtime.spawn_blocking(move || {
                shared.diagnostics.log(&format!(
                    "Found {} Runs left over from the last launch",
                    leftover.len()
                ));
                for line in leftovers::clean_up(&leftover, Duration::from_secs(3)) {
                    shared.diagnostics.log(&line);
                }
            });
        }
        supervisor
    }

    // MARK: - Discovered Processes

    /// Looks for other Node and Bun processes now, not in a few seconds.
    pub fn rescan(&self) {
        self.shared.rescan.notify_one();
    }

    /// Other processes › Stop…: sends SIGTERM to the top process of a
    /// Discovered Process, if it is still the same process.
    pub fn stop_discovered(&self, pid: u32, start: u64) -> Result<()> {
        if !crate::sys::is_same_process(pid as i32, start) {
            return Err(Error::Invalid(format!("PID {pid} has already exited.")));
        }
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid as i32),
            nix::sys::signal::Signal::SIGTERM,
        )
        .map_err(|e| Error::Invalid(format!("Couldn't send SIGTERM to PID {pid}: {e}")))?;
        self.shared
            .diagnostics
            .log(&format!("Sent SIGTERM to Discovered Process {pid}"));
        self.shared.lock().exiting.insert((pid, start));
        self.shared.notifier.notify();
        self.rescan();
        Ok(())
    }

    /// The Force Kill alert: SIGKILL to every process of a Discovered
    /// Process that is still the same process.
    pub fn force_kill_discovered(&self, pid: u32, start: u64) -> Result<()> {
        let process = self
            .shared
            .lock()
            .discovered
            .iter()
            .find(|p| p.pid == pid && p.start == start)
            .cloned();
        let (members, starts) = match process {
            Some(p) => (p.members, p.member_starts),
            None => (vec![pid], vec![start]),
        };
        for (member, member_start) in members.into_iter().zip(starts) {
            if crate::sys::is_same_process(member as i32, member_start) {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(member as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
        }
        self.shared
            .diagnostics
            .log(&format!("Force killed Discovered Process {pid}"));
        self.rescan();
        Ok(())
    }

    /// Quit: stops every live Run. Returns how many there were; the app
    /// waits for the snapshot to show none before it quits.
    pub fn stop_all(&self) -> usize {
        let mut state = self.shared.lock();
        let mut count = 0;
        for run in state.runs.values_mut().filter(|r| r.status.is_live()) {
            count += 1;
            run.quitting = true;
            run.stop_requested = true;
            run.restart_after = false;
            if let Some(control) = &run.control {
                let _ = control.send(Control::Stop);
            }
            run.status = RunStatus::Stopping;
        }
        drop(state);
        self.shared
            .diagnostics
            .log(&format!("Quitting: stopping {count} Runs"));
        self.shared.notifier.notify();
        count
    }

    // MARK: - Environment

    /// Reads the login shell's environment again, in the background. New
    /// Runs get it; running ones keep theirs.
    pub fn reload_environment(&self) {
        {
            let mut state = self.shared.lock();
            if state.environment.loading {
                return;
            }
            state.environment.loading = true;
        }
        self.shared.notifier.notify();
        let shared = self.shared.clone();
        self.runtime.spawn(async move {
            let captured = match &shared.login_shell {
                Some(shell) => {
                    env::capture(shell, shared.environment_timeout, &shared.spawn_lock).await
                }
                None => Ok(std::env::vars_os().collect()),
            };
            {
                let mut state = shared.lock();
                match captured {
                    Ok(vars) => {
                        state.base_env = vars;
                        state.environment.problem = None;
                    }
                    Err(reason) => {
                        state.environment.problem = Some(format!(
                            "Couldn't read the environment ({reason}). Runs get Bunode's own."
                        ));
                    }
                }
                state.environment.loaded_at = Some(SystemTime::now());
            }
            resolve_tools(&shared).await;
            shared.lock().environment.loading = false;
            shared.notifier.notify();
        });
    }

    /// Advanced › Node.js / Bun › Change…: use this executable instead of
    /// the one in PATH. `None` goes back to PATH.
    pub fn set_tool_path(&self, tool: Tool, path: Option<&Path>) -> Result<()> {
        if let Some(path) = path
            && !env::is_executable(path)
        {
            return Err(Error::Invalid(format!(
                "{} is not an executable file.",
                path.display()
            )));
        }
        let path = path.map(Path::to_path_buf);
        self.update_settings(|settings| match tool {
            Tool::Node => settings.node_path = path,
            Tool::Bun => settings.bun_path = path,
        })?;
        let shared = self.shared.clone();
        self.runtime.spawn(async move {
            resolve_tools(&shared).await;
            shared.notifier.notify();
        });
        Ok(())
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
            environment: EnvironmentState {
                shell: self
                    .shared
                    .login_shell
                    .as_ref()
                    .map(|s| s.display().to_string())
                    .unwrap_or_default(),
                variable_count: state.base_env.len() as u32,
                loaded_at: state.environment.loaded_at.map(|t| {
                    t.duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0)
                }),
                is_loading: state.environment.loading,
                problem: state.environment.problem.clone(),
                tools: state.environment.tools.clone(),
            },
            config_issue: state.config_issue.clone(),
            discovered: state
                .discovered
                .iter()
                .map(|p| DiscoveredProcess {
                    exiting: state.exiting.contains(&(p.pid, p.start)),
                    ..p.clone()
                })
                .collect(),
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
        let options = |scripts: &[package::Script], folder: Option<&Path>| -> Vec<ScriptOption> {
            scripts
                .iter()
                .map(|script| {
                    let command = package::script_command(manager, &script.name);
                    ScriptOption {
                        used: used(&command, folder),
                        name: script.name.clone(),
                        command,
                    }
                })
                .collect()
        };
        let mut groups = vec![ScriptGroup {
            package_name: None,
            folder: None,
            scripts: options(scripts.as_deref().unwrap_or_default(), None),
        }];
        // Monorepo packages run their scripts in their own folder.
        for package in crate::workspace::packages(&path) {
            let folder = PathBuf::from(&package.folder);
            groups.push(ScriptGroup {
                scripts: options(&package.scripts, Some(&folder)),
                package_name: Some(package.name),
                folder: Some(package.folder),
            });
        }
        Ok(ProjectScripts {
            has_package_json: scripts.is_some(),
            groups,
        })
    }

    /// Adds a Service to a Project. Nothing is started.
    pub fn add_service(&self, project_id: ProjectId, new: NewService) -> Result<ServiceState> {
        let mut state = self.shared.lock();
        let project = state
            .config
            .project(project_id)
            .ok_or(Error::UnknownProject)?;
        let command = new.command.trim();
        if command.is_empty() {
            return Err(Error::Invalid("Enter a command.".into()));
        }
        let folder = validate_folder(&project.path, new.folder.as_deref())?;
        let name = validate_service_name(project, &new.name, folder.as_deref(), None)?;

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

    /// Advanced › Force stop after: 3, 5, 10 or 30 seconds. Applies to Runs
    /// started from now on.
    pub fn set_force_stop_after(&self, seconds: u32) -> Result<()> {
        if !FORCE_STOP_CHOICES.contains(&seconds) {
            return Err(Error::Invalid(format!(
                "Force stop after must be one of {FORCE_STOP_CHOICES:?} seconds."
            )));
        }
        self.update_settings(|settings| settings.force_stop_after_secs = seconds)
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

    /// Settings › Projects › package manager: `None` is Auto (detected).
    /// Applies to Services added from now on and their Runs.
    pub fn set_package_manager(
        &self,
        project_id: ProjectId,
        manager: Option<PackageManager>,
    ) -> Result<()> {
        let mut state = self.shared.lock();
        let mut config = state.config.clone();
        let project = config
            .project_mut(project_id)
            .ok_or(Error::UnknownProject)?;
        if project.package_manager == manager {
            return Ok(());
        }
        project.package_manager = manager;
        self.shared.save(&config)?;
        state.config = config;
        drop(state);
        self.shared.notifier.notify();
        Ok(())
    }

    /// Settings › Projects › Remove…: removes a Project and its Services from
    /// Bunode. Live Runs are stopped first; the Project goes once the last
    /// one has ended. Files in its folder are never touched.
    pub fn remove_project(&self, project_id: ProjectId) -> Result<()> {
        let mut state = self.shared.lock();
        let project = state
            .config
            .project(project_id)
            .ok_or(Error::UnknownProject)?;
        let services: Vec<ServiceId> = project.services.iter().map(|s| s.id).collect();
        let mut waiting = false;
        for id in &services {
            if let Some(run) = state.runs.get_mut(id)
                && run.status.is_live()
            {
                waiting = true;
                run.delete_after = true;
                run.restart_after = false;
                run.stop_requested = true;
                if let Some(control) = &run.control {
                    let _ = control.send(Control::Stop);
                }
                run.status = RunStatus::Stopping;
            }
        }
        state.removing_projects.insert(project_id);
        drop(state);
        // Services without a live Run go at once.
        for id in services {
            let live = self
                .shared
                .lock()
                .runs
                .get(&id)
                .is_some_and(|r| r.status.is_live());
            if !live {
                self.shared.remove_service(id)?;
            }
        }
        if !waiting {
            self.shared.remove_project_if_done(project_id)?;
        }
        self.shared.notifier.notify();
        Ok(())
    }

    /// Home › Locate…: the Project's folder moved; point the Project at
    /// its new place and detect its package manager again.
    pub fn relocate_project(&self, project_id: ProjectId, path: &Path) -> Result<()> {
        let not_a_folder = || Error::NotAFolder {
            path: path.display().to_string(),
        };
        let path = fs::canonicalize(path).map_err(|_| not_a_folder())?;
        if !path.is_dir() {
            return Err(not_a_folder());
        }
        let detection = package::detect(&path);
        let mut state = self.shared.lock();
        if state
            .config
            .projects
            .iter()
            .any(|p| p.id != project_id && p.path == path)
        {
            return Err(Error::Invalid(format!(
                "{} is already a Project.",
                path.display()
            )));
        }
        let mut config = state.config.clone();
        let project = config
            .project_mut(project_id)
            .ok_or(Error::UnknownProject)?;
        project.path = path;
        self.shared.save(&config)?;
        state.config = config;
        state.detections.insert(project_id, detection);
        drop(state);
        self.shared.notifier.notify();
        Ok(())
    }

    /// Saves the Service view's settings. A live Run keeps running with the
    /// old ones; they apply to the next Run.
    pub fn update_service(&self, service_id: ServiceId, edit: ServiceEdit) -> Result<()> {
        let mut state = self.shared.lock();
        let (project, _) = find_service(&state.config, service_id)?;
        let command = edit.command.trim().to_owned();
        if command.is_empty() {
            return Err(Error::Invalid("Enter a command.".into()));
        }
        if !edit.run_through_shell {
            launch::parse_command(&command).map_err(Error::Invalid)?;
        }
        if edit.port == Some(0) {
            return Err(Error::Invalid("Use a port from 1 to 65535.".into()));
        }
        let folder = validate_folder(&project.path, edit.folder.as_deref())?;
        let project_id = project.id;

        let mut config = state.config.clone();
        let service = config
            .project_mut(project_id)
            .and_then(|p| p.services.iter_mut().find(|s| s.id == service_id))
            .ok_or(Error::UnknownService)?;
        let before = service.clone();
        service.command = command;
        service.folder = folder;
        service.port = edit.port;
        service.run_through_shell = edit.run_through_shell;
        if *service == before {
            return Ok(());
        }
        self.shared.save(&config)?;
        state.config = config;
        if let Some(run) = state.runs.get_mut(&service_id)
            && run.status.is_live()
        {
            run.config_changed = true;
        }
        drop(state);
        self.shared.notifier.notify();
        Ok(())
    }

    /// Deletes a Service from Bunode. A live Run is stopped first; the
    /// configuration goes once the Supervisor has seen the Run end. The
    /// Project's files are never touched; the Service's own log files are.
    pub fn delete_service(&self, service_id: ServiceId) -> Result<()> {
        let mut state = self.shared.lock();
        find_service(&state.config, service_id)?;
        if let Some(run) = state.runs.get_mut(&service_id)
            && run.status.is_live()
        {
            run.delete_after = true;
            run.restart_after = false;
            run.stop_requested = true;
            if let Some(control) = &run.control {
                let _ = control.send(Control::Stop);
            }
            run.status = RunStatus::Stopping;
            drop(state);
            self.shared.notifier.notify();
            return Ok(());
        }
        drop(state);
        self.shared.remove_service(service_id)
    }

    // MARK: - Runs

    /// Starts a Run of the Service, unless one is already live. Returns at
    /// once; the Run's progress arrives through snapshots.
    pub fn start(&self, service_id: ServiceId) -> Result<()> {
        self.shared.start(service_id)
    }

    /// Restarts a running Service: its Run is stopped completely, process
    /// group and all, then a new Run starts. Restarting while a restart is
    /// under way does nothing. A Service that isn't running just starts.
    pub fn restart(&self, service_id: ServiceId) -> Result<()> {
        let mut state = self.shared.lock();
        let Some(run) = state.runs.get_mut(&service_id) else {
            drop(state);
            return self.shared.start(service_id);
        };
        match run.status {
            RunStatus::Restarting => return Ok(()),
            _ if run.delete_after => return Ok(()),
            RunStatus::Starting | RunStatus::Running | RunStatus::Stopping => {
                if let Some(control) = &run.control {
                    let _ = control.send(Control::Stop);
                }
                run.stop_requested = true;
                run.restart_after = true;
                run.status = RunStatus::Restarting;
            }
            RunStatus::Stopped | RunStatus::Crashed | RunStatus::Failed => {
                drop(state);
                return self.shared.start(service_id);
            }
        }
        drop(state);
        self.shared.notifier.notify();
        Ok(())
    }

    // MARK: - Logs

    /// Up to `max` lines of the Service's current or last Run, from `from` on.
    pub fn log_lines(&self, service_id: ServiceId, from: u64, max: u32) -> Vec<LogLine> {
        let logs = self
            .shared
            .lock()
            .runs
            .get(&service_id)
            .map(|run| run.logs.clone());
        match logs {
            Some(logs) => lock_logs(&logs).lines(from, max as usize),
            None => Vec::new(),
        }
    }

    /// Logs › Search: shows only lines containing `query`, ignoring case.
    /// Returns at once; the matches arrive through snapshots, and new lines
    /// that match are added as they come.
    pub fn set_log_filter(&self, service_id: ServiceId, query: &str) {
        let logs = {
            let mut state = self.shared.lock();
            if query.trim().is_empty() {
                state.log_filters.remove(&service_id);
            } else {
                state.log_filters.insert(service_id, query.to_owned());
            }
            state.runs.get(&service_id).map(|run| run.logs.clone())
        };
        let Some(logs) = logs else { return };
        let shared = self.shared.clone();
        let query = query.to_owned();
        self.runtime.spawn_blocking(move || {
            lock_logs(&logs).set_filter(&query);
            shared.notifier.notify();
        });
    }

    /// Up to `max` lines matching the search, from match number `from` on.
    pub fn log_matching_lines(&self, service_id: ServiceId, from: u64, max: u32) -> Vec<LogLine> {
        let logs = self
            .shared
            .lock()
            .runs
            .get(&service_id)
            .map(|run| run.logs.clone());
        match logs {
            Some(logs) => lock_logs(&logs).matching_lines(from, max as usize),
            None => Vec::new(),
        }
    }

    /// Logs › Export…: writes the full log of the Service's current or last
    /// Run, not only the lines kept in memory, to `destination`.
    pub fn export_log(&self, service_id: ServiceId, destination: &Path) -> Result<()> {
        let (run_number, file) = {
            let state = self.shared.lock();
            let run = state
                .runs
                .get(&service_id)
                .ok_or_else(|| Error::Invalid("This Service has no Runs yet.".into()))?;
            (run.run_number, run.file.clone())
        };
        if let Some(file) = file {
            file.flush(Duration::from_secs(2));
        }
        self.shared
            .log_files
            .export(service_id, run_number, destination)
            .map(|_| ())
            .map_err(|e| {
                Error::Invalid(format!("Couldn't export to {}: {e}", destination.display()))
            })
    }

    /// Logs › Clear: hides the lines shown so far. Files are not touched.
    pub fn clear_logs(&self, service_id: ServiceId) {
        let logs = self
            .shared
            .lock()
            .runs
            .get(&service_id)
            .map(|run| run.logs.clone());
        if let Some(logs) = logs {
            lock_logs(&logs).clear();
            self.shared.notifier.notify();
        }
    }

    /// Advanced › Log lines kept: 5,000, 10,000 or 50,000 per Service, for
    /// Runs started from now on.
    pub fn set_log_lines_kept(&self, lines: u32) -> Result<()> {
        if !LOG_LINES_CHOICES.contains(&lines) {
            return Err(Error::Invalid(format!(
                "Log lines kept must be one of {LOG_LINES_CHOICES:?}."
            )));
        }
        self.update_settings(|settings| settings.log_lines_kept = lines)
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
        // Stopping wins over a pending restart.
        run.restart_after = false;
        if matches!(
            run.status,
            RunStatus::Starting | RunStatus::Running | RunStatus::Restarting
        ) {
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
    /// Starts a Run of the Service, unless one is already live.
    fn start(self: &Arc<Self>, service_id: ServiceId) -> Result<()> {
        let mut state = self.lock();
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
        let command = service.command.clone();
        let logs = Arc::new(Mutex::new(LogBuffer::new(
            state.config.settings.log_lines_kept as usize,
        )));
        if let Some(query) = state.log_filters.get(&service_id) {
            lock_logs(&logs).set_filter(query);
        }

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
        let _ = self.store.save(&config);
        state.config = config;
        let log = RunLog {
            buffer: logs.clone(),
            file: RunLogWriter::start(&self.log_files, service_id, run_number, logs.clone()),
        };
        log.system(&format!("Run #{run_number}: {command}"));

        let spec = match spec {
            Ok(spec) => spec,
            Err(message) => {
                log.system(&message);
                state.runs.insert(
                    service_id,
                    RunEntry {
                        run_number,
                        status: RunStatus::Failed,
                        pid: None,
                        stop_requested: false,
                        control: None,
                        last_exit: Some(message),
                        logs,
                        file: Some(log.file.clone()),
                        restart_after: false,
                        delete_after: false,
                        config_changed: false,
                        quitting: false,
                        ended_by: Some(RunEnd::FailedToStart),
                    },
                );
                drop(state);
                self.notifier.notify();
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
                logs: logs.clone(),
                file: Some(log.file.clone()),
                restart_after: false,
                delete_after: false,
                config_changed: false,
                quitting: false,
                ended_by: None,
            },
        );
        drop(state);
        self.notifier.notify();

        let shared = self.clone();
        self.runtime.spawn(async move {
            let outcome = run::drive(
                shared.clone(),
                service_id,
                run_number,
                spec,
                log,
                control_rx,
            )
            .await;
            shared.finish(service_id, run_number, outcome);
        });
        Ok(())
    }

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

    /// Removes a Service that has no live Run, and its Project when that
    /// was being removed and this was its last Service.
    fn remove_service(&self, service_id: ServiceId) -> Result<()> {
        let mut state = self.lock();
        let project_id = state
            .config
            .projects
            .iter()
            .find(|p| p.services.iter().any(|s| s.id == service_id))
            .map(|p| p.id);
        let mut config = state.config.clone();
        for project in &mut config.projects {
            project.services.retain(|s| s.id != service_id);
        }
        self.save(&config)?;
        state.config = config;
        state.runs.remove(&service_id);
        state.log_filters.remove(&service_id);
        drop(state);
        self.log_files.remove_service(service_id);
        if let Some(project_id) = project_id {
            self.remove_project_if_done(project_id)?;
        }
        self.notifier.notify();
        Ok(())
    }

    /// Removes a Project being removed once none of its Services is left.
    fn remove_project_if_done(&self, project_id: ProjectId) -> Result<()> {
        let mut state = self.lock();
        if !state.removing_projects.contains(&project_id) {
            return Ok(());
        }
        let empty = state
            .config
            .project(project_id)
            .is_none_or(|p| p.services.is_empty());
        if !empty {
            return Ok(());
        }
        let mut config = state.config.clone();
        config.projects.retain(|p| p.id != project_id);
        self.save(&config)?;
        state.config = config;
        state.detections.remove(&project_id);
        state.removing_projects.remove(&project_id);
        Ok(())
    }

    /// The runner of a Run was spawned: remember it on disk.
    pub(crate) fn runner_spawned(&self, service_id: ServiceId, run_number: u64, pid: u32) {
        let Some(info) = crate::sys::process(pid as i32) else {
            return;
        };
        self.records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(RunRecord {
                service: service_id,
                run: run_number,
                runner_pid: pid as i32,
                runner_start: info.start,
                group: None,
                group_start: None,
            });
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
        self.records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_group(service_id, run_number, pid as i32);
        self.notifier.notify();
    }

    /// The Run is over; record how it ended, and start the next one if this
    /// was a restart.
    fn finish(self: &Arc<Self>, service_id: ServiceId, run_number: u64, outcome: Outcome) {
        let mut state = self.lock();
        let Some(run) = state.runs.get_mut(&service_id) else {
            return;
        };
        if run.run_number != run_number {
            return;
        }
        let (status, note) = outcome.classify(run.stop_requested);
        run.ended_by = Some(match status {
            RunStatus::Failed => RunEnd::FailedToStart,
            RunStatus::Crashed => RunEnd::Crashed,
            _ if run.quitting => RunEnd::BunodeQuit,
            _ if run.stop_requested => RunEnd::StoppedByUser,
            _ => RunEnd::Exited,
        });
        let line = match (&note, run.quitting) {
            (None, true) => "Stopped because Bunode quit".to_owned(),
            (None, false) => "Stopped".to_owned(),
            (Some(note), _) => note.clone(),
        };
        let time = now_ms();
        if let Some(file) = &run.file {
            file.line(time, LogStream::System, &line);
        }
        lock_logs(&run.logs).push_system(time, line);
        run.status = status;
        run.last_exit = note;
        run.pid = None;
        run.control = None;
        run.config_changed = false;
        let restart = std::mem::take(&mut run.restart_after);
        let delete = std::mem::take(&mut run.delete_after);
        let ended_by = run.ended_by;
        drop(state);
        self.records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(service_id, run_number);
        self.diagnostics.log(&format!(
            "Service {service_id} Run #{run_number} ended: {ended_by:?}"
        ));
        if delete {
            let _ = self.remove_service(service_id);
        } else if restart {
            let _ = self.start(service_id);
        }
        self.notifier.notify();
    }
}

/// Scans for Discovered Processes every few seconds, or when asked.
async fn discover(shared: Arc<Shared>) {
    let scanner = Arc::new(Mutex::new(Scanner::default()));
    loop {
        let ours = {
            let state = shared.lock();
            Ours {
                groups: state
                    .runs
                    .values()
                    .filter_map(|r| r.pid)
                    .map(|p| p as i32)
                    .collect(),
                runners: Default::default(),
            }
        };
        let scanning = scanner.clone();
        let Ok(found) = tokio::task::spawn_blocking(move || {
            scanning
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .scan(&ours)
        })
        .await
        else {
            return;
        };
        let changed = {
            let mut state = shared.lock();
            let alive: std::collections::HashSet<(u32, u64)> =
                found.iter().map(|p| (p.pid, p.start)).collect();
            state.exiting.retain(|key| alive.contains(key));
            let changed = state.discovered != found;
            state.discovered = found;
            changed
        };
        if changed {
            shared.notifier.notify();
        }
        let _ = tokio::time::timeout(DISCOVERY_INTERVAL, shared.rescan.notified()).await;
    }
}

/// Finds `node` and `bun`, the user's picks first, and asks their versions.
async fn resolve_tools(shared: &Shared) {
    let (base_env, settings) = {
        let state = shared.lock();
        (state.base_env.clone(), state.config.settings.clone())
    };
    let mut tools = Vec::new();
    for tool in [Tool::Node, Tool::Bun] {
        let custom = match tool {
            Tool::Node => settings.node_path.clone(),
            Tool::Bun => settings.bun_path.clone(),
        };
        let is_custom = custom.is_some();
        let path = match custom {
            Some(path) => env::is_executable(&path).then_some(path),
            None => env::which(&base_env, tool.executable()),
        };
        let version = match &path {
            Some(path) => env::version(path, &shared.spawn_lock).await,
            None => None,
        };
        tools.push(ToolState {
            tool,
            path: path.map(|p| p.display().to_string()),
            version,
            is_custom,
        });
    }
    shared.lock().environment.tools = tools;
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

/// Checks that `folder` is the Project's directory or inside it, symlinks
/// and `..` resolved. Returns it relative to the Project; `None` for the
/// directory itself.
fn validate_folder(project: &Path, folder: Option<&str>) -> Result<Option<PathBuf>> {
    let Some(folder) = folder.map(str::trim).filter(|f| !f.is_empty() && *f != ".") else {
        return Ok(None);
    };
    let outside = || Error::Invalid("Choose the Project's folder or a folder inside it.".into());
    let root = fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    let candidate = if Path::new(folder).is_absolute() {
        PathBuf::from(folder)
    } else {
        root.join(folder)
    };
    let resolved = fs::canonicalize(&candidate)
        .map_err(|_| Error::Invalid(format!("{} doesn't exist.", candidate.display())))?;
    let relative = resolved.strip_prefix(&root).map_err(|_| outside())?;
    if relative.as_os_str().is_empty() {
        return Ok(None);
    }
    if !resolved.is_dir() {
        return Err(Error::NotAFolder {
            path: resolved.display().to_string(),
        });
    }
    Ok(Some(relative.to_path_buf()))
}

/// Checks a Service name: 1–40 characters and unique among the Project's
/// Services in the same folder, ignoring case (a monorepo has a `dev` in
/// many packages). Returns it trimmed.
fn validate_service_name(
    project: &Project,
    name: &str,
    folder: Option<&Path>,
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
    let taken = project.services.iter().any(|s| {
        Some(s.id) != except
            && s.folder.as_deref() == folder
            && s.name.to_lowercase() == name.to_lowercase()
    });
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
    let logs = run.map(|r| lock_logs(&r.logs).info()).unwrap_or_default();
    // A Service in a subfolder shows the folder's name first: "web › dev".
    let display_name = match service.folder.as_deref().and_then(Path::file_name) {
        Some(folder) => format!("{} › {}", folder.to_string_lossy(), service.name),
        None => service.name.clone(),
    };
    ServiceState {
        id: service.id,
        project_id: project.id,
        name: service.name.clone(),
        display_name,
        command: service.command.clone(),
        folder: service.folder.as_ref().map(|f| f.display().to_string()),
        port: service.port,
        run_through_shell: service.run_through_shell,
        status: run.map_or(RunStatus::Stopped, |r| r.status),
        run_number: run.map(|r| r.run_number),
        pid: run.and_then(|r| r.pid),
        last_exit: run.and_then(|r| r.last_exit.clone()),
        log_first: logs.first,
        log_count: logs.count,
        log_dropped: logs.dropped,
        log_match_first: logs.match_first,
        log_match_count: logs.match_count,
        log_filter_generation: logs.filter_generation,
        has_pending_config: run.is_some_and(|r| r.config_changed && r.status.is_live()),
        ended_by: run.and_then(|r| r.ended_by),
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
        is_missing: !project.path.is_dir(),
        services: project
            .services
            .iter()
            .map(|s| service_state(project, s, state.runs.get(&s.id)))
            .collect(),
    }
}

/// Calls the listener on its own thread, coalescing bursts of changes.
/// `notify` is cheap enough to call for every log line.
pub(crate) struct Notifier {
    tx: mpsc::Sender<()>,
    pending: Arc<AtomicBool>,
}

impl Notifier {
    fn new(listener: Arc<dyn Listener>) -> Notifier {
        let (tx, rx) = mpsc::channel::<()>();
        let pending = Arc::new(AtomicBool::new(false));
        let flag = pending.clone();
        thread::Builder::new()
            .name("bunode-notify".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    flag.store(false, Ordering::SeqCst);
                    listener.state_changed();
                    thread::sleep(COALESCE);
                }
            })
            .expect("spawn notifier thread");
        Notifier { tx, pending }
    }

    pub(crate) fn notify(&self) {
        if !self.pending.swap(true, Ordering::SeqCst) {
            let _ = self.tx.send(());
        }
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
        let mut options = SupervisorOptions::new(data, "/nonexistent/bunode-runner");
        options.login_shell = Some("/bin/sh".into());
        let supervisor = Supervisor::new(options, counter.clone());
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
        let settings = supervisor.snapshot().settings;
        assert!(settings.show_running_count);
        assert_eq!(settings.force_stop_after_secs, 5);
        supervisor.set_show_running_count(false).unwrap();
        supervisor.set_force_stop_after(10).unwrap();
        assert!(supervisor.set_force_stop_after(7).is_err());
        drop(supervisor);
        let (reloaded, _) = open(data.path());
        let settings = reloaded.snapshot().settings;
        assert!(!settings.show_running_count);
        assert_eq!(settings.force_stop_after_secs, 10);
    }

    fn wait_until(what: &str, done: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !done() {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for {what}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn reads_the_login_shell_environment_and_finds_tools() {
        let data = tempfile::tempdir().unwrap();
        let (supervisor, _) = open(data.path());
        wait_until("the environment", || {
            supervisor.snapshot().environment.loaded_at.is_some()
                && !supervisor.snapshot().environment.is_loading
        });
        let environment = supervisor.snapshot().environment;
        assert_eq!(environment.shell, "/bin/sh");
        assert!(environment.variable_count > 0);
        assert_eq!(environment.problem, None);
        assert_eq!(
            environment.tools.iter().map(|t| t.tool).collect::<Vec<_>>(),
            [Tool::Node, Tool::Bun]
        );
    }

    #[test]
    fn custom_tool_paths_are_checked_and_kept() {
        let data = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        let fake = tools.path().join("node");
        fs::write(&fake, "not executable").unwrap();
        let (supervisor, _) = open(data.path());
        assert!(supervisor.set_tool_path(Tool::Node, Some(&fake)).is_err());
        crate::env::write_test_executable(&fake, "#!/bin/sh\necho v99.1.0\n");
        supervisor.set_tool_path(Tool::Node, Some(&fake)).unwrap();
        wait_until("the custom node", || {
            supervisor
                .snapshot()
                .environment
                .tools
                .first()
                .is_some_and(|t| t.is_custom && t.version.as_deref() == Some("99.1.0"))
        });
        let node = supervisor.snapshot().environment.tools[0].clone();
        assert_eq!(node.path, Some(fake.display().to_string()));

        supervisor.set_tool_path(Tool::Node, None).unwrap();
        wait_until("node from PATH", || {
            supervisor
                .snapshot()
                .environment
                .tools
                .first()
                .is_some_and(|t| !t.is_custom)
        });
    }

    fn edit(command: &str) -> ServiceEdit {
        ServiceEdit {
            command: command.into(),
            folder: None,
            port: None,
            run_through_shell: false,
        }
    }

    #[test]
    fn edits_are_checked_and_saved() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        fs::create_dir_all(code.path().join("apps/web")).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, code.path());
        let id = supervisor
            .add_service(project.id, new_service("dev", "bun run dev"))
            .unwrap()
            .id;

        assert!(supervisor.update_service(id, edit("")).is_err());
        assert!(supervisor.update_service(id, edit("node 'oops")).is_err());
        let mut shell = edit("node 'oops");
        shell.run_through_shell = true;
        assert!(
            supervisor.update_service(id, shell).is_ok(),
            "the shell parses it"
        );
        let mut zero = edit("x");
        zero.port = Some(0);
        assert!(supervisor.update_service(id, zero).is_err());

        let mut escape = edit("x");
        escape.folder = Some("../".into());
        assert!(supervisor.update_service(id, escape).is_err());
        let mut absolute = edit("x");
        absolute.folder = Some(outside.path().display().to_string());
        assert!(supervisor.update_service(id, absolute).is_err());

        let mut good = edit("vite --port 4321");
        good.folder = Some("apps/web/".into());
        good.port = Some(4321);
        supervisor.update_service(id, good).unwrap();
        drop(supervisor);

        let (reloaded, _) = open(data.path());
        let service = reloaded.snapshot().projects[0].services[0].clone();
        assert_eq!(service.command, "vite --port 4321");
        assert_eq!(service.folder.as_deref(), Some("apps/web"));
        assert_eq!(service.port, Some(4321));
        assert!(!service.has_pending_config);
    }

    #[test]
    fn deleting_a_stopped_service_removes_it_and_keeps_files() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        fs::write(code.path().join("package.json"), "{}").unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, code.path());
        let id = supervisor
            .add_service(project.id, new_service("dev", "bun run dev"))
            .unwrap()
            .id;
        supervisor.delete_service(id).unwrap();
        assert!(supervisor.snapshot().projects[0].services.is_empty());
        assert!(code.path().join("package.json").exists());
        assert!(matches!(
            supervisor.delete_service(id),
            Err(Error::UnknownService)
        ));
    }

    #[test]
    fn picking_a_package_manager_clears_the_lockfile_warning() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        fs::write(
            code.path().join("package.json"),
            r#"{"scripts": {"dev": "vite"}}"#,
        )
        .unwrap();
        fs::write(code.path().join("yarn.lock"), "").unwrap();
        fs::write(code.path().join("package-lock.json"), "").unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, code.path());
        assert!(project.several_lockfiles);

        supervisor
            .set_package_manager(project.id, Some(PackageManager::Npm))
            .unwrap();
        let project = supervisor.snapshot().projects[0].clone();
        assert!(!project.several_lockfiles);
        assert_eq!(project.package_manager, PackageManager::Npm);
        assert_eq!(project.detected_package_manager, PackageManager::Yarn);
        let scripts = supervisor.project_scripts(project.id).unwrap();
        assert_eq!(scripts.groups[0].scripts[0].command, "npm run dev");

        supervisor.set_package_manager(project.id, None).unwrap();
        assert!(supervisor.snapshot().projects[0].several_lockfiles);
    }

    #[test]
    fn removing_a_project_keeps_its_files() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        fs::write(code.path().join("package.json"), "{}").unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, code.path());
        supervisor
            .add_service(project.id, new_service("dev", "bun run dev"))
            .unwrap();
        supervisor.remove_project(project.id).unwrap();
        assert!(supervisor.snapshot().projects.is_empty());
        assert!(code.path().join("package.json").exists());
        drop(supervisor);
        assert!(open(data.path()).0.snapshot().projects.is_empty());
    }

    #[test]
    fn a_moved_folder_is_missing_until_located() {
        let data = tempfile::tempdir().unwrap();
        let code = tempfile::tempdir().unwrap();
        let old = code.path().join("api");
        fs::create_dir(&old).unwrap();
        let (supervisor, _) = open(data.path());
        let project = added_project(&supervisor, &old);
        assert!(!project.is_missing);

        let new = code.path().join("api-moved");
        fs::rename(&old, &new).unwrap();
        assert!(supervisor.snapshot().projects[0].is_missing);

        supervisor.relocate_project(project.id, &new).unwrap();
        let located = supervisor.snapshot().projects[0].clone();
        assert!(!located.is_missing);
        assert!(located.path.ends_with("api-moved"));
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
