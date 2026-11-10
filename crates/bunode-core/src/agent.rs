//! Agent Servers (requirements §3.10, ADR-0007): dev servers a coding agent
//! started from its launch settings in a Project's root. Bunode shows them
//! next to the Project's Services; it never claims, starts or logs them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;

use crate::discovery::Details;
use crate::model::ProjectId;
use crate::sys::ProcessInfo;

/// One configuration of an agent's launch file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfig {
    pub name: String,
    pub runtime_executable: String,
    #[serde(default)]
    pub runtime_args: Vec<String>,
}

impl LaunchConfig {
    /// `runtimeExecutable` and `runtimeArgs`, as the row's tooltip shows them.
    pub fn command(&self) -> String {
        std::iter::once(self.runtime_executable.as_str())
            .chain(self.runtime_args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Whether `argv`, what the launcher runs, is this configuration: the
    /// same executable by file name, then exactly the same arguments.
    fn matches(&self, argv: &[String]) -> bool {
        let Some((executable, args)) = argv.split_first() else {
            return false;
        };
        file_name(executable) == file_name(&self.runtime_executable) && args == self.runtime_args
    }
}

fn file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// The configurations of a launch file (`{version, configurations: [...]}`).
pub fn parse_launch_file(text: &str) -> Result<Vec<LaunchConfig>, serde_json::Error> {
    #[derive(Deserialize)]
    struct LaunchFile {
        #[serde(default)]
        configurations: Vec<LaunchConfig>,
    }
    serde_json::from_str::<LaunchFile>(text).map(|f| f.configurations)
}

/// A supported agent: what its launcher looks like and where it keeps its
/// launch settings. One row per agent (ADR-0007).
#[derive(Debug, PartialEq, Eq)]
pub struct Adapter {
    /// The agent's name as the UI shows it: "Claude".
    pub name: &'static str,
    /// The launch file, relative to the Project's root.
    pub launch_file: &'static str,
    /// The launcher's executable, by the end of its path.
    launcher: &'static str,
    /// The executable of the launcher's parent, by the end of its path.
    parent: &'static str,
    /// An argument before `--` that marks a launch that is no dev server.
    excluded_flag: &'static str,
}

pub const ADAPTERS: &[Adapter] = &[Adapter {
    name: "Claude",
    launch_file: ".claude/launch.json",
    launcher: "Claude.app/Contents/Helpers/disclaimer",
    parent: "Claude.app/Contents/MacOS/Claude",
    // MCP servers.
    excluded_flag: "--pgroup",
}];

/// A Project whose agent launch settings are watched.
#[derive(Debug, Clone)]
pub struct Watched {
    pub project_id: ProjectId,
    pub root: PathBuf,
    pub adapter: &'static Adapter,
    pub configs: Vec<LaunchConfig>,
}

/// An Agent Server found by a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentServer {
    pub project_id: ProjectId,
    /// "Claude".
    pub agent: &'static str,
    /// The configuration's `name`.
    pub name: String,
    /// The configuration's `runtimeExecutable` and `runtimeArgs`.
    pub command: String,
    /// The launcher, which leads the process group.
    pub pid: u32,
    pub start: u64,
    pub pgid: u32,
    /// TCP ports any process of the group listens on.
    pub ports: Vec<u16>,
}

/// The Agent Servers in `infos`: Projects in `watched` order, then their
/// configurations in launch-file order, then oldest first. `details` holds
/// the arguments and working directory of the user's processes.
pub(crate) fn detect(
    infos: &HashMap<i32, ProcessInfo>,
    details: &HashMap<i32, Details>,
    watched: &[Watched],
) -> Vec<AgentServer> {
    let args_of = |pid: i32| details.get(&pid).and_then(|d| d.args.as_deref());
    let mut found: Vec<(usize, usize, AgentServer)> = Vec::new();
    for info in infos.values() {
        // The launcher leads its own process group.
        if info.pgid != info.pid {
            continue;
        }
        let Some(args) = args_of(info.pid) else {
            continue;
        };
        let Some(launcher) = args.first().map(Path::new) else {
            continue;
        };
        let Some(dashes) = args.iter().position(|a| a == "--") else {
            continue;
        };
        let (flags, argv) = (&args[1..dashes], &args[dashes + 1..]);
        let cwd = details.get(&info.pid).and_then(|d| d.cwd.as_deref());
        let parent = args_of(info.ppid).and_then(|a| a.first()).map(Path::new);
        for (w, project) in watched.iter().enumerate() {
            let adapter = project.adapter;
            if !launcher.ends_with(adapter.launcher)
                || !parent.is_some_and(|p| p.ends_with(adapter.parent))
                || flags.iter().any(|f| f == adapter.excluded_flag)
                || cwd.map(Path::new) != Some(project.root.as_path())
            {
                continue;
            }
            let Some((c, config)) = project
                .configs
                .iter()
                .enumerate()
                .find(|(_, c)| c.matches(argv))
            else {
                continue;
            };
            found.push((
                w,
                c,
                AgentServer {
                    project_id: project.project_id,
                    agent: adapter.name,
                    name: config.name.clone(),
                    command: config.command(),
                    pid: info.pid as u32,
                    start: info.start,
                    pgid: info.pgid as u32,
                    ports: Vec::new(),
                },
            ));
            break;
        }
    }
    found.sort_by_key(|(w, c, server)| (*w, *c, server.start, server.pid));
    found.into_iter().map(|(_, _, server)| server).collect()
}

/// The launch files of the Projects, each read when its Project is added
/// and then again only once it changed.
#[derive(Debug, Default)]
pub struct LaunchFiles {
    files: HashMap<(ProjectId, &'static str), LaunchFile>,
}

#[derive(Debug)]
struct LaunchFile {
    root: PathBuf,
    modified: Option<SystemTime>,
    configs: Vec<LaunchConfig>,
}

impl LaunchFiles {
    /// Reads `root`'s launch files that changed since they were last read.
    /// Returns whether any did, and a diagnostics line for each that can't
    /// be parsed (it then counts as having no configurations).
    pub fn refresh(&mut self, project_id: ProjectId, root: &Path) -> (bool, Vec<String>) {
        let mut changed = false;
        let mut problems = Vec::new();
        for adapter in ADAPTERS {
            let path = root.join(adapter.launch_file);
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            let key = (project_id, adapter.name);
            if let Some(file) = self.files.get(&key)
                && file.root == root
                && file.modified == modified
            {
                continue;
            }
            let configs = match modified.map(|_| std::fs::read_to_string(&path)) {
                None => Vec::new(),
                Some(Ok(text)) => parse_launch_file(&text).unwrap_or_else(|e| {
                    problems.push(format!("Couldn't parse {}: {e}", path.display()));
                    Vec::new()
                }),
                Some(Err(e)) => {
                    problems.push(format!("Couldn't read {}: {e}", path.display()));
                    Vec::new()
                }
            };
            changed |= self
                .files
                .get(&key)
                .map_or(!configs.is_empty(), |f| f.configs != configs);
            self.files.insert(
                key,
                LaunchFile {
                    root: root.to_owned(),
                    modified,
                    configs,
                },
            );
        }
        (changed, problems)
    }

    /// Forgets a removed Project's launch files.
    pub fn forget(&mut self, project_id: ProjectId) {
        self.files.retain(|(id, _), _| *id != project_id);
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// What a scan looks for, Projects in `order`.
    pub fn watched(&self, order: &[ProjectId]) -> Vec<Watched> {
        order
            .iter()
            .flat_map(|&id| {
                ADAPTERS.iter().filter_map(move |adapter| {
                    let file = self.files.get(&(id, adapter.name))?;
                    (!file.configs.is_empty()).then(|| Watched {
                        project_id: id,
                        root: file.root.clone(),
                        adapter,
                        configs: file.configs.clone(),
                    })
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAUNCH_FILE: &str = r#"{
        "version": "0.0.1",
        "configurations": [
            { "name": "web", "runtimeExecutable": "npm", "runtimeArgs": ["run", "dev"], "port": 3001 },
            { "name": "api", "runtimeExecutable": "/opt/homebrew/bin/bun", "runtimeArgs": ["run", "api"],
              "env": { "PORT": "4000" } },
            { "name": "plain", "runtimeExecutable": "node" }
        ]
    }"#;

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reads_the_configurations_of_a_launch_file() {
        let configs = parse_launch_file(LAUNCH_FILE).unwrap();
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0].name, "web");
        assert_eq!(configs[0].command(), "npm run dev");
        assert_eq!(configs[1].runtime_executable, "/opt/homebrew/bin/bun");
        assert!(configs[2].runtime_args.is_empty());
        assert_eq!(parse_launch_file(r#"{"version":"0.0.1"}"#).unwrap(), []);
        assert!(parse_launch_file("{ not json").is_err());
        assert!(parse_launch_file(r#"{"configurations":[{"name":"x"}]}"#).is_err());
    }

    #[test]
    fn a_configuration_matches_by_executable_name_and_exact_arguments() {
        let configs = parse_launch_file(LAUNCH_FILE).unwrap();
        let web = &configs[0];
        assert!(web.matches(&words(&["npm", "run", "dev"])));
        assert!(web.matches(&words(&["/opt/homebrew/bin/npm", "run", "dev"])));
        assert!(!web.matches(&words(&["npm", "run", "dev", "--host"])));
        assert!(!web.matches(&words(&["npm", "run"])));
        assert!(!web.matches(&words(&["pnpm", "run", "dev"])));
        assert!(!web.matches(&[]));
        assert!(configs[1].matches(&words(&["bun", "run", "api"])));
    }

    /// A fake process table: Claude, its launchers, and whatever they run.
    struct Table {
        infos: HashMap<i32, ProcessInfo>,
        details: HashMap<i32, Details>,
    }

    const CLAUDE: &str = "/Applications/Claude.app/Contents/MacOS/Claude";
    const DISCLAIMER: &str = "/Applications/Claude.app/Contents/Helpers/disclaimer";

    impl Table {
        fn new() -> Table {
            let mut table = Table {
                infos: HashMap::new(),
                details: HashMap::new(),
            };
            table.add(100, 1, 100, &[CLAUDE], Some("/"));
            table
        }

        fn add(&mut self, pid: i32, ppid: i32, pgid: i32, args: &[&str], cwd: Option<&str>) {
            self.infos.insert(
                pid,
                ProcessInfo {
                    pid,
                    ppid,
                    pgid,
                    uid: 501,
                    start: pid as u64,
                    name: file_name(args[0]).to_owned(),
                },
            );
            self.details.insert(
                pid,
                Details {
                    args: Some(words(args)),
                    cwd: cwd.map(str::to_owned),
                },
            );
        }

        /// A launcher Claude started, in its own group, running `command`.
        fn launch(&mut self, pid: i32, cwd: &str, command: &[&str]) {
            let args: Vec<&str> = [DISCLAIMER, "--"]
                .into_iter()
                .chain(command.iter().copied())
                .collect();
            self.add(pid, 100, pid, &args, Some(cwd));
        }

        fn detect(&self, watched: &[Watched]) -> Vec<(ProjectId, String, u32)> {
            detect(&self.infos, &self.details, watched)
                .into_iter()
                .map(|s| (s.project_id, s.name, s.pid))
                .collect()
        }
    }

    fn watched(project_id: ProjectId, root: &str) -> Watched {
        Watched {
            project_id,
            root: root.into(),
            adapter: &ADAPTERS[0],
            configs: parse_launch_file(LAUNCH_FILE).unwrap(),
        }
    }

    #[test]
    fn finds_a_server_claude_launched_in_a_project_root() {
        let mut table = Table::new();
        table.launch(200, "/code/shop", &["npm", "run", "dev"]);
        // npm and the server it runs share the launcher's group.
        table.add(
            201,
            200,
            200,
            &["node", "/x/npm-cli.js", "run", "dev"],
            Some("/code/shop"),
        );
        let servers = detect(&table.infos, &table.details, &[watched(7, "/code/shop")]);
        assert_eq!(servers.len(), 1);
        let server = &servers[0];
        assert_eq!(
            (server.project_id, server.agent, server.name.as_str()),
            (7, "Claude", "web")
        );
        assert_eq!(server.command, "npm run dev");
        assert_eq!((server.pid, server.pgid), (200, 200));
    }

    #[test]
    fn only_all_three_rules_together_make_an_agent_server() {
        let mut table = Table::new();
        // A worktree and a subfolder of the Project aren't its root.
        table.launch(
            201,
            "/code/shop/.claude/worktrees/x",
            &["npm", "run", "dev"],
        );
        table.launch(202, "/code/shop/web", &["npm", "run", "dev"]);
        // An MCP server.
        table.add(
            203,
            100,
            203,
            &[DISCLAIMER, "--pgroup", "--", "npm", "run", "dev"],
            Some("/code/shop"),
        );
        // Not one of the configurations.
        table.launch(204, "/code/shop", &["npm", "run", "build"]);
        // The same command, but not from Claude: from a terminal, and from
        // a launcher whose parent isn't Claude.
        table.add(205, 1, 205, &["npm", "run", "dev"], Some("/code/shop"));
        table.add(
            206,
            1,
            206,
            &[DISCLAIMER, "--", "npm", "run", "dev"],
            Some("/code/shop"),
        );
        // A launcher that doesn't lead its own group.
        table.add(
            207,
            100,
            100,
            &[DISCLAIMER, "--", "npm", "run", "dev"],
            Some("/code/shop"),
        );
        // No `--`.
        table.add(
            208,
            100,
            208,
            &[DISCLAIMER, "npm", "run", "dev"],
            Some("/code/shop"),
        );
        assert_eq!(table.detect(&[watched(7, "/code/shop")]), []);
    }

    #[test]
    fn servers_are_listed_by_project_then_configuration_then_age() {
        let mut table = Table::new();
        table.launch(300, "/code/shop", &["bun", "run", "api"]);
        table.launch(301, "/code/blog", &["npm", "run", "dev"]);
        table.launch(302, "/code/shop", &["npm", "run", "dev"]);
        // The same configuration launched twice: both show.
        table.launch(303, "/code/shop", &["npm", "run", "dev"]);
        let found = table.detect(&[watched(1, "/code/shop"), watched(2, "/code/blog")]);
        assert_eq!(
            found,
            [
                (1, "web".to_owned(), 302),
                (1, "web".to_owned(), 303),
                (1, "api".to_owned(), 300),
                (2, "web".to_owned(), 301),
            ]
        );
        // A Project nobody watches has none.
        assert_eq!(table.detect(&[watched(2, "/code/blog")]).len(), 1);
    }

    #[test]
    fn a_launch_file_is_read_again_only_once_it_changed() {
        let root = tempfile::tempdir().unwrap();
        let mut files = LaunchFiles::default();
        // No launch file: nothing to watch.
        assert_eq!(files.refresh(1, root.path()), (false, vec![]));
        assert!(files.watched(&[1]).is_empty());

        std::fs::create_dir(root.path().join(".claude")).unwrap();
        let path = root.path().join(".claude/launch.json");
        std::fs::write(&path, LAUNCH_FILE).unwrap();
        assert_eq!(files.refresh(1, root.path()), (true, vec![]));
        assert_eq!(files.watched(&[1])[0].configs.len(), 3);
        assert_eq!(files.refresh(1, root.path()), (false, vec![]));

        // Unparseable: no configurations, and one diagnostics line.
        std::fs::write(&path, "{ not json").unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(later)
            .unwrap();
        let (changed, problems) = files.refresh(1, root.path());
        assert!(changed);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("launch.json"), "{problems:?}");
        assert!(files.watched(&[1]).is_empty());

        files.forget(1);
        assert!(files.watched(&[1]).is_empty());
    }

    #[test]
    fn the_vscode_launch_file_is_never_read() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".vscode")).unwrap();
        std::fs::write(root.path().join(".vscode/launch.json"), LAUNCH_FILE).unwrap();
        let mut files = LaunchFiles::default();
        files.refresh(1, root.path());
        assert!(files.watched(&[1]).is_empty());
    }
}
