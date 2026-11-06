//! Node.js and Bun processes Bunode did not start (requirements §3.3).
//! They are only shown and, on request, sent SIGTERM; never restarted,
//! never claimed as a Service's Run, never signalled in bulk.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::sys::{self, ProcessInfo};

/// A Discovered Process: a Node or Bun process tree started outside
/// Bunode, shown as one item (`npm run dev` and the Vite it started).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredProcess {
    /// The top of the tree: the process `Stop…` signals.
    pub pid: u32,
    /// With `pid`, what identifies it (see `sys::ProcessInfo::start`).
    pub start: u64,
    /// "npm run dev", "node server.js".
    pub command: String,
    pub cwd: Option<String>,
    /// Where it was started from: "Terminal", "VS Code", "tmux"…
    pub source: String,
    /// Every process of the tree, the top one first, and their start
    /// times, so a force kill can check each one is still the same.
    pub members: Vec<u32>,
    pub member_starts: Vec<u64>,
    /// TCP ports any of them listens on.
    pub ports: Vec<u16>,
    /// SIGTERM was sent and it has not exited yet.
    pub exiting: bool,
}

/// Process names that are JavaScript runtimes or their launchers.
const LAUNCHERS: &[&str] = &["npm", "npx", "pnpm", "pnpx", "yarn", "yarnpkg", "bunx"];

fn is_javascript(info: &ProcessInfo, args: Option<&[String]>) -> bool {
    let name = info.name.as_str();
    if name == "node" || name == "bun" {
        return true;
    }
    // Node rewrites its title, e.g. to "npm run dev".
    let first = name.split_whitespace().next().unwrap_or("");
    if LAUNCHERS.contains(&first) {
        return true;
    }
    args.and_then(|a| a.first())
        .map(|a| basename(a))
        .is_some_and(|b| b == "node" || b == "bun")
}

fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// "npm run dev" rather than "node /opt/homebrew/lib/node_modules/npm/bin/npm-cli.js run dev".
pub fn display_command(args: &[String]) -> String {
    let Some(first) = args.first() else {
        return String::new();
    };
    // A rewritten title is one argument with spaces.
    if args.len() == 1 && first.contains(' ') {
        return first.clone();
    }
    let runtime = basename(first);
    if (runtime == "node" || runtime == "bun")
        && let Some(script) = args.get(1)
    {
        let tool = basename(script)
            .trim_end_matches(".js")
            .trim_end_matches(".cjs")
            .trim_end_matches(".mjs")
            .trim_end_matches("-cli");
        if LAUNCHERS.contains(&tool) {
            return std::iter::once(tool.to_owned())
                .chain(args[2..].iter().cloned())
                .collect::<Vec<_>>()
                .join(" ");
        }
    }
    std::iter::once(runtime.to_owned())
        .chain(args[1..].iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The app a process tree was started from, by its ancestors' names.
pub fn source_of(ancestors: &[&ProcessInfo]) -> String {
    const SHELLS: &[&str] = &[
        "zsh", "bash", "sh", "fish", "login", "dash", "nu", "-zsh", "-bash",
    ];
    const KNOWN: &[(&str, &str)] = &[
        ("terminal", "Terminal"),
        ("iterm", "iTerm"),
        ("warp", "Warp"),
        ("ghostty", "Ghostty"),
        ("wezterm", "WezTerm"),
        ("kitty", "kitty"),
        ("alacritty", "Alacritty"),
        ("tmux", "tmux"),
        ("screen", "screen"),
        ("cursor", "Cursor"),
        ("code helper", "VS Code"),
        ("electron", "VS Code"),
        ("code", "VS Code"),
        ("zed", "Zed"),
        ("webstorm", "WebStorm"),
        ("idea", "IntelliJ IDEA"),
        ("launchd", "launchd"),
    ];
    for ancestor in ancestors {
        let name = ancestor.name.to_lowercase();
        if SHELLS.contains(&name.as_str()) {
            continue;
        }
        if let Some((_, label)) = KNOWN.iter().find(|(key, _)| name.contains(key)) {
            return (*label).to_owned();
        }
        return ancestor.name.clone();
    }
    "Unknown".into()
}

/// What stays the same while a process lives, so it is read once.
#[derive(Clone)]
struct Details {
    args: Option<Vec<String>>,
    cwd: Option<String>,
}

/// Scans the machine for Discovered Processes, remembering what it read.
#[derive(Default)]
pub struct Scanner {
    details: HashMap<(i32, u64), Details>,
}

impl Scanner {
    /// One scan. `ours` are the process groups and runners of Bunode's own
    /// Runs, which are never listed; the ports each of those groups listens
    /// on are returned alongside.
    pub fn scan(&mut self, ours: &Ours) -> Scan {
        let uid = sys::current_uid();
        let me = std::process::id() as i32;
        let infos: HashMap<i32, ProcessInfo> = sys::all_pids()
            .into_iter()
            .filter_map(sys::process)
            .map(|p| (p.pid, p))
            .collect();

        // Forget processes that are gone.
        self.details
            .retain(|(pid, start), _| infos.get(pid).is_some_and(|p| p.start == *start));

        let mut javascript: HashSet<i32> = HashSet::new();
        for info in infos.values().filter(|p| p.uid == uid && p.pid != me) {
            let details = self.details_of(info);
            if is_javascript(info, details.args.as_deref()) {
                javascript.insert(info.pid);
            }
        }

        let ancestors = |pid: i32| -> Vec<&ProcessInfo> {
            let mut chain = Vec::new();
            let mut seen = HashSet::new();
            let mut current = infos.get(&pid).map(|p| p.ppid);
            while let Some(parent) = current {
                if parent <= 0 || !seen.insert(parent) {
                    break;
                }
                let Some(info) = infos.get(&parent) else {
                    break;
                };
                chain.push(info);
                current = Some(info.ppid);
            }
            chain
        };
        let is_ours = |info: &ProcessInfo| {
            ours.groups.contains(&info.pgid)
                || ours.runners.contains(&info.pid)
                || info.pid == me
                || ancestors(info.pid)
                    .iter()
                    .any(|a| ours.runners.contains(&a.pid) || a.pid == me)
        };

        // Every JavaScript process belongs to its topmost JavaScript
        // ancestor, shells in between allowed (npm runs scripts via sh).
        let mut trees: HashMap<i32, Vec<i32>> = HashMap::new();
        for &pid in &javascript {
            let info = &infos[&pid];
            if is_ours(info) {
                continue;
            }
            let root = ancestors(pid)
                .iter()
                .rev()
                .find(|a| javascript.contains(&a.pid))
                .map_or(pid, |a| a.pid);
            trees.entry(root).or_default().push(pid);
        }

        let mut found: Vec<DiscoveredProcess> = trees
            .into_iter()
            .map(|(root, mut members)| {
                members.sort_unstable();
                members.retain(|&m| m != root);
                members.insert(0, root);
                let info = &infos[&root];
                let details = self.details_of(info);
                let command = details
                    .args
                    .as_deref()
                    .map(display_command)
                    .unwrap_or_else(|| info.name.clone());
                let mut ports: Vec<u16> = members
                    .iter()
                    .flat_map(|&m| sys::listening_ports(m))
                    .collect();
                ports.sort_unstable();
                ports.dedup();
                DiscoveredProcess {
                    pid: root as u32,
                    start: info.start,
                    command,
                    cwd: details.cwd,
                    source: source_of(&ancestors(root)),
                    member_starts: members.iter().map(|m| infos[m].start).collect(),
                    members: members.into_iter().map(|m| m as u32).collect(),
                    ports,
                    exiting: false,
                }
            })
            .collect();
        found.sort_by_key(|p| (p.start, p.pid));

        // The ports of Bunode's own Runs: every process of each group.
        let mut group_ports: HashMap<i32, Vec<u16>> = HashMap::new();
        for info in infos.values().filter(|p| ours.groups.contains(&p.pgid)) {
            group_ports
                .entry(info.pgid)
                .or_default()
                .extend(sys::listening_ports(info.pid));
        }
        for ports in group_ports.values_mut() {
            ports.sort_unstable();
            ports.dedup();
        }
        Scan {
            discovered: found,
            group_ports,
        }
    }

    fn details_of(&mut self, info: &ProcessInfo) -> Details {
        self.details
            .entry((info.pid, info.start))
            .or_insert_with(|| Details {
                args: sys::arguments(info.pid),
                cwd: sys::cwd(info.pid).map(|p| p.display().to_string()),
            })
            .clone()
    }
}

/// What one scan found.
pub struct Scan {
    pub discovered: Vec<DiscoveredProcess>,
    /// The TCP ports each of Bunode's own Run groups listens on.
    pub group_ports: HashMap<i32, Vec<u16>>,
}

/// Bunode's own processes, never listed.
#[derive(Debug, Default, Clone)]
pub struct Ours {
    pub groups: HashSet<i32>,
    pub runners: HashSet<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn shows_launchers_by_their_name() {
        assert_eq!(
            display_command(&args(&[
                "node",
                "/opt/homebrew/lib/node_modules/npm/bin/npm-cli.js",
                "run",
                "dev"
            ])),
            "npm run dev"
        );
        assert_eq!(
            display_command(&args(&[
                "/usr/local/bin/node",
                "/usr/local/bin/pnpm",
                "dev"
            ])),
            "pnpm dev"
        );
        assert_eq!(
            display_command(&args(&["/Users/me/.bun/bin/bun", "run", "dev"])),
            "bun run dev"
        );
        assert_eq!(
            display_command(&args(&["node", "server.js", "--port", "3000"])),
            "node server.js --port 3000"
        );
        assert_eq!(display_command(&args(&["npm run dev"])), "npm run dev");
        assert_eq!(
            display_command(&args(&["npm run tree -- /tmp/x/pids"])),
            "npm run tree -- /tmp/x/pids"
        );
    }

    fn info(pid: i32, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid: 0,
            pgid: pid,
            uid: 501,
            start: 1,
            name: name.into(),
        }
    }

    #[test]
    fn names_the_app_it_came_from() {
        let zsh = info(10, "zsh");
        let terminal = info(9, "Terminal");
        let tmux = info(8, "tmux");
        let code = info(7, "Code Helper (Plugin)");
        assert_eq!(source_of(&[&zsh, &terminal]), "Terminal");
        assert_eq!(source_of(&[&zsh, &tmux, &terminal]), "tmux");
        assert_eq!(source_of(&[&code]), "VS Code");
        assert_eq!(source_of(&[&zsh, &info(1, "launchd")]), "launchd");
        assert_eq!(source_of(&[&zsh]), "Unknown");
        assert_eq!(source_of(&[&info(3, "my-runner")]), "my-runner");
    }
}
