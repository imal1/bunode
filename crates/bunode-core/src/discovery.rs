//! Node.js and Bun processes Bunode did not start (requirements §3.3).
//! They are only shown and, on request, sent SIGTERM; never restarted,
//! never claimed as a Service's Run, never signalled in bulk.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::agent::{self, AgentServer, Watched};
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
    /// The app it was started from; `None` when unknown.
    pub source: Option<String>,
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

/// Whether an executable named `name` is node, bun or one of their launchers.
pub(crate) fn is_runtime_or_launcher(name: &str) -> bool {
    name == "node" || name == "bun" || LAUNCHERS.contains(&name)
}

fn is_javascript(info: &ProcessInfo, args: Option<&[String]>) -> bool {
    // Node rewrites its title, e.g. to "npm run dev".
    let first = info.name.split_whitespace().next().unwrap_or("");
    if is_runtime_or_launcher(first) {
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

/// The app a process tree was started from, by its ancestors' names;
/// `None` when only shells started it.
pub fn source_of(ancestors: &[&ProcessInfo]) -> Option<String> {
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
            return Some((*label).to_owned());
        }
        return Some(ancestor.name.clone());
    }
    None
}

/// What stays the same while a process lives, so it is read once.
#[derive(Debug, Clone)]
pub(crate) struct Details {
    pub(crate) args: Option<Vec<String>>,
    pub(crate) cwd: Option<String>,
}

/// Scans the machine for Discovered Processes, remembering what it read.
#[derive(Default)]
pub struct Scanner {
    details: HashMap<(i32, u64), Details>,
}

impl Scanner {
    /// One scan. `ours` are the process groups and runners of Bunode's own
    /// Runs, which are never listed; the ports each of those groups listens
    /// on are returned alongside. The Agent Servers of `watched` are
    /// returned too, and their processes aren't Discovered Processes.
    pub fn scan(&mut self, ours: &Ours, watched: &[Watched]) -> Scan {
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

        let details: HashMap<i32, Details> = infos
            .values()
            .filter(|p| p.uid == uid && p.pid != me)
            .map(|info| (info.pid, self.details_of(info)))
            .collect();
        let mut agent_servers = agent::detect(&infos, &details, watched);
        let agent_groups: HashSet<i32> = agent_servers.iter().map(|s| s.pgid as i32).collect();
        let javascript: HashSet<i32> = details
            .iter()
            .filter(|(pid, d)| is_javascript(&infos[pid], d.args.as_deref()))
            .map(|(&pid, _)| pid)
            .collect();

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
                || agent_groups.contains(&info.pgid)
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
                let details = &details[&root];
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
                    cwd: details.cwd.clone(),
                    source: source_of(&ancestors(root)),
                    member_starts: members.iter().map(|m| infos[m].start).collect(),
                    members: members.into_iter().map(|m| m as u32).collect(),
                    ports,
                    exiting: false,
                }
            })
            .collect();
        found.sort_by_key(|p| (p.start, p.pid));

        // The ports of Bunode's own Runs and of the Agent Servers: every
        // process of each group.
        let mut group_ports: HashMap<i32, Vec<u16>> = HashMap::new();
        for info in infos
            .values()
            .filter(|p| ours.groups.contains(&p.pgid) || agent_groups.contains(&p.pgid))
        {
            group_ports
                .entry(info.pgid)
                .or_default()
                .extend(sys::listening_ports(info.pid));
        }
        for ports in group_ports.values_mut() {
            ports.sort_unstable();
            ports.dedup();
        }
        for server in &mut agent_servers {
            server.ports = group_ports
                .remove(&(server.pgid as i32))
                .unwrap_or_default();
        }
        Scan {
            discovered: found,
            group_ports,
            agent_servers,
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

/// Who holds a port Bunode was about to use (design-handoff §5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortOwner {
    pub pid: u32,
    /// "astro dev"
    pub command: String,
    /// "Terminal"
    /// The app it was started from; `None` when unknown.
    pub source: Option<String>,
}

/// The user's process listening on TCP `port`, if any. Reads every process
/// of the user, so it is for a Start, not for a loop.
pub fn port_owner(port: u16) -> Option<PortOwner> {
    let uid = sys::current_uid();
    let infos: HashMap<i32, ProcessInfo> = sys::all_pids()
        .into_iter()
        .filter_map(sys::process)
        .map(|p| (p.pid, p))
        .collect();
    let owner = infos
        .values()
        .filter(|p| p.uid == uid)
        .find(|p| sys::listening_ports(p.pid).contains(&port))?;
    let mut ancestors = Vec::new();
    let mut current = infos.get(&owner.ppid);
    while let Some(info) = current {
        if ancestors.len() > 64 || info.pid <= 1 && !ancestors.is_empty() {
            ancestors.push(info);
            break;
        }
        ancestors.push(info);
        current = infos.get(&info.ppid).filter(|p| p.pid != info.pid);
    }
    let command = sys::arguments(owner.pid)
        .as_deref()
        .map(display_command)
        .unwrap_or_else(|| owner.name.clone());
    Some(PortOwner {
        pid: owner.pid as u32,
        command,
        source: source_of(&ancestors),
    })
}

/// Whether something already listens on TCP `port` on this Mac: a
/// connection to it on the loopback address, IPv4 or IPv6, is accepted.
/// (Trying to bind the port instead gives false alarms on macOS for a
/// moment after a listener closes.)
pub fn port_in_use(port: u16) -> bool {
    !probe_port(port).is_empty()
}

/// The loopback connections to `port` that were accepted, as
/// "local -> peer". A connection to itself (the kernel picked `port` as
/// the source port too) means nobody listens, and doesn't count.
pub fn probe_port(port: u16) -> Vec<String> {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
    use std::time::Duration;
    let targets: [SocketAddr; 2] = [
        (Ipv4Addr::LOCALHOST, port).into(),
        (Ipv6Addr::LOCALHOST, port).into(),
    ];
    targets
        .iter()
        .filter_map(|addr| {
            let stream = TcpStream::connect_timeout(addr, Duration::from_millis(300)).ok()?;
            let (local, peer) = (stream.local_addr().ok()?, stream.peer_addr().ok()?);
            (local != peer).then(|| format!("{local} -> {peer}"))
        })
        .collect()
}

/// The first free port after `port`, for `Use :N`.
pub fn next_free_port(port: u16) -> Option<u16> {
    (port.checked_add(1)?..=port.saturating_add(200)).find(|&p| !port_in_use(p))
}

/// What one scan found.
pub struct Scan {
    pub discovered: Vec<DiscoveredProcess>,
    /// The TCP ports each of Bunode's own Run groups listens on.
    pub group_ports: HashMap<i32, Vec<u16>>,
    pub agent_servers: Vec<AgentServer>,
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
    fn only_node_and_bun_processes_are_discovered() {
        assert!(is_javascript(&info(1, "node"), None));
        assert!(is_javascript(&info(1, "npm run dev"), None));
        let node = args(&["/opt/node/bin/node", "server.js"]);
        assert!(is_javascript(&info(1, "MainThread"), Some(&node)));
        // Services may run these (ADR-0005), but they are never Discovered Processes.
        let qq = args(&["/Applications/QQ.app/Contents/MacOS/QQ", "--no-sandbox"]);
        assert!(!is_javascript(&info(1, "QQ"), Some(&qq)));
        let python = args(&["python3", "-m", "http.server"]);
        assert!(!is_javascript(&info(1, "Python"), Some(&python)));
    }

    #[test]
    fn knows_who_holds_a_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(port_in_use(port));
        let owner = port_owner(port).expect("this test process");
        assert_eq!(owner.pid, std::process::id());
        // (Whether `next` is still free a moment later is up to other tests.)
        let next = next_free_port(port).unwrap();
        assert!(next > port);
        // Whether the port is free again right after `drop` can't be checked
        // here: on macOS a child that another test spawns at the same moment
        // can inherit the listening socket before it is close-on-exec. The
        // integration test `a_port_held_outside_bunode_names_its_process`
        // checks a port freeing up when its process exits.
        drop(listener);
    }

    #[test]
    fn names_the_app_it_came_from() {
        let zsh = info(10, "zsh");
        let terminal = info(9, "Terminal");
        let tmux = info(8, "tmux");
        let code = info(7, "Code Helper (Plugin)");
        let source = |a: &[&ProcessInfo]| source_of(a);
        assert_eq!(source(&[&zsh, &terminal]).as_deref(), Some("Terminal"));
        assert_eq!(source(&[&zsh, &tmux, &terminal]).as_deref(), Some("tmux"));
        assert_eq!(source(&[&code]).as_deref(), Some("VS Code"));
        assert_eq!(
            source(&[&zsh, &info(1, "launchd")]).as_deref(),
            Some("launchd")
        );
        assert_eq!(source(&[&zsh]), None);
        assert_eq!(
            source(&[&info(3, "my-runner")]).as_deref(),
            Some("my-runner")
        );
    }
}
