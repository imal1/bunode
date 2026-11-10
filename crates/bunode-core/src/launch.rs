//! Turns a Service's configuration into what `bunode-runner` executes.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::model::{Project, Service, Settings};
use crate::snapshot::RunNote;

/// Everything needed to start one Run.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(OsString, OsString)>,
    pub stop_timeout: Duration,
}

/// Builds the launch spec of `service`. `base_env` is the environment Runs
/// start from. Fails when the command can't be run.
pub fn launch_spec(
    project: &Project,
    service: &Service,
    settings: &Settings,
    base_env: &[(OsString, OsString)],
) -> Result<LaunchSpec, RunNote> {
    let mut env: Vec<(OsString, OsString)> = base_env
        .iter()
        // Injected libraries of a debugged Bunode must not leak into Runs.
        .filter(|(key, _)| !key.to_string_lossy().starts_with("DYLD_"))
        .cloned()
        .collect();
    // Most dev tools turn colors off when output is not a terminal.
    if !env.iter().any(|(k, _)| k == "FORCE_COLOR") {
        env.push(("FORCE_COLOR".into(), "1".into()));
    }
    // Node.js and Bun picked in Settings come first in PATH, for commands
    // that run on them (ADR-0005).
    let custom_dirs: Vec<PathBuf> = [&settings.node_path, &settings.bun_path]
        .into_iter()
        .flatten()
        .filter_map(|tool| tool.parent().map(Path::to_path_buf))
        .collect();
    let cwd = match &service.folder {
        Some(folder) => project.path.join(folder),
        None => project.path.clone(),
    };
    let path = crate::env::get(&env, "PATH").map(OsStr::to_os_string);
    if !custom_dirs.is_empty() && may_use_javascript_runtime(service, &cwd, path.as_deref()) {
        let dirs = custom_dirs
            .into_iter()
            .chain(path.iter().flat_map(std::env::split_paths));
        if let Ok(path) = std::env::join_paths(dirs) {
            set_env(&mut env, "PATH".into(), path);
        }
    }
    for (key, value) in &service.env {
        set_env(&mut env, key.into(), value.into());
    }

    let (program, args) = if service.run_through_shell {
        let shell = env
            .iter()
            .find(|(k, _)| k == "SHELL")
            .map(|(_, v)| v.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/zsh".into());
        (shell, vec!["-c".to_owned(), service.command.clone()])
    } else {
        let mut words = parse_command(&service.command).map_err(|e| match e {
            crate::Error::UnmatchedQuote => RunNote::UnmatchedQuote,
            crate::Error::EmptyCommand => RunNote::EmptyCommand,
            other => unreachable!("parse_command doesn't return {other:?}"),
        })?;
        let program = words.remove(0);
        (program, words)
    };

    Ok(LaunchSpec {
        program,
        args,
        cwd,
        env,
        stop_timeout: Duration::from_secs(u64::from(settings.force_stop_after_secs.max(1))),
    })
}

/// Splits a command into the executable and its arguments, with shell-like
/// quoting but no shell features (pipes, `&&`, globs, variables).
pub fn parse_command(command: &str) -> crate::Result<Vec<String>> {
    let words = shell_words::split(command).map_err(|_| crate::Error::UnmatchedQuote)?;
    if words.is_empty() {
        return Err(crate::Error::EmptyCommand);
    }
    Ok(words)
}

/// Whether `service` may run on Node.js or Bun. It doesn't when its
/// executable is found to be something else: a native binary, or a script
/// for another interpreter such as Python. A shell may run them anywhere,
/// and a name not in `path` may be in a Runtime Path, so those count.
fn may_use_javascript_runtime(service: &Service, cwd: &Path, path: Option<&OsStr>) -> bool {
    if service.run_through_shell {
        return true;
    }
    let Some(program) = shell_words::split(&service.command)
        .ok()
        .and_then(|words| words.into_iter().next())
    else {
        return true;
    };
    let program = Path::new(&program);
    let name = program.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if crate::discovery::is_runtime_or_launcher(name)
        || program
            .components()
            .any(|c| c.as_os_str() == "node_modules")
    {
        return true;
    }
    let file = if program.components().count() > 1 {
        Some(cwd.join(program))
    } else {
        path.into_iter()
            .flat_map(std::env::split_paths)
            .map(|dir| dir.join(program))
            .find(|file| file.is_file())
    };
    file.is_none_or(|file| runs_on_javascript_or_shell(&file))
}

/// Whether the script at `file` names Node.js, Bun or a shell in its `#!`
/// line. Native binaries and files that can't be read don't.
fn runs_on_javascript_or_shell(file: &Path) -> bool {
    let mut head = [0; 256];
    let Ok(read) = std::fs::File::open(file).and_then(|mut f| f.read(&mut head)) else {
        return false;
    };
    let Some(line) = head[..read].strip_prefix(b"#!") else {
        return false;
    };
    let line = String::from_utf8_lossy(line.split(|&b| b == b'\n').next().unwrap_or_default());
    let mut words = line.split_whitespace();
    let mut interpreter = words.next().and_then(|w| w.rsplit('/').next());
    if interpreter == Some("env") {
        interpreter = words.find(|w| !w.starts_with('-') && !w.contains('='));
    }
    matches!(
        interpreter,
        Some("node" | "bun" | "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish")
    )
}

pub fn set_env(env: &mut Vec<(OsString, OsString)>, key: OsString, value: OsString) {
    match env.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => env.push((key, value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Project {
        Project {
            id: 1,
            name: "api".into(),
            alias: None,
            path: "/code/api".into(),
            package_manager: None,
            services: vec![],
        }
    }

    fn service(command: &str) -> Service {
        Service {
            id: 2,
            name: "dev".into(),
            command: command.into(),
            folder: None,
            port: None,
            run_through_shell: false,
            env: Default::default(),
            runs: 0,
        }
    }

    fn env(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs.iter().map(|(k, v)| (k.into(), v.into())).collect()
    }

    #[test]
    fn splits_commands_with_quotes() {
        let spec = launch_spec(
            &project(),
            &service(r#"node server.js --name "my app" 'it''s'"#),
            &Settings::default(),
            &[],
        )
        .unwrap();
        assert_eq!(spec.program, "node");
        assert_eq!(spec.args, ["server.js", "--name", "my app", "its"]);
        assert_eq!(spec.cwd, PathBuf::from("/code/api"));
        assert_eq!(spec.stop_timeout, Duration::from_secs(5));
    }

    #[test]
    fn splits_like_a_shell_would_without_running_one() {
        let words = |c: &str| parse_command(c).unwrap();
        assert_eq!(words(r"node a\ b.js"), ["node", "a b.js"]);
        assert_eq!(words(r#"node "say \"hi\"""#), ["node", r#"say "hi""#]);
        assert_eq!(words("node '$HOME' \"*.js\""), ["node", "$HOME", "*.js"]);
        assert_eq!(words("a && b | c"), ["a", "&&", "b", "|", "c"]);
        assert_eq!(words("  vite   --host  "), ["vite", "--host"]);
    }

    #[test]
    fn shell_mode_runs_through_the_users_shell() {
        let mut s = service("vite && echo done | tee x");
        s.run_through_shell = true;
        let spec = launch_spec(
            &project(),
            &s,
            &Settings::default(),
            &env(&[("SHELL", "/bin/bash")]),
        )
        .unwrap();
        assert_eq!(spec.program, "/bin/bash");
        assert_eq!(spec.args, ["-c", "vite && echo done | tee x"]);
    }

    #[test]
    fn rejects_unparsable_commands() {
        let settings = Settings::default();
        assert_eq!(
            launch_spec(&project(), &service("node 'oops"), &settings, &[]),
            Err(RunNote::UnmatchedQuote)
        );
        assert_eq!(
            launch_spec(&project(), &service("   "), &settings, &[]),
            Err(RunNote::EmptyCommand)
        );
    }

    #[test]
    fn custom_node_and_bun_come_first_in_path() {
        let settings = Settings {
            node_path: Some("/opt/node/bin/node".into()),
            bun_path: Some("/opt/bun/bin/bun".into()),
            ..Settings::default()
        };
        let spec = launch_spec(
            &project(),
            &service("node x.js"),
            &settings,
            &env(&[("PATH", "/usr/bin:/bin")]),
        )
        .unwrap();
        let path = spec
            .env
            .iter()
            .find(|(k, _)| k == "PATH")
            .unwrap()
            .1
            .clone();
        assert_eq!(
            path,
            OsString::from("/opt/node/bin:/opt/bun/bin:/usr/bin:/bin")
        );
    }

    #[test]
    fn runtime_paths_apply_to_node_and_bun_commands_only() {
        let dir = tempfile::tempdir().unwrap();
        let script = |name: &str, first_line: &str| {
            std::fs::write(dir.path().join(name), format!("{first_line}\necho hi\n")).unwrap();
        };
        script("server.js", "#!/usr/bin/env node");
        script("tsx", "#!/usr/bin/env -S node --no-warnings");
        script("dev.sh", "#!/bin/sh");
        script("serve.py", "#!/usr/bin/env python3");
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::fs::copy("/bin/ls", bin.join("tool")).unwrap();
        let project = Project {
            path: dir.path().into(),
            ..project()
        };
        let settings = Settings {
            node_path: Some("/opt/node/bin/node".into()),
            ..Settings::default()
        };
        let base_path = format!("/usr/bin:{}", bin.display());
        let path_of = |command: &str, shell: bool| {
            let mut service = service(command);
            service.run_through_shell = shell;
            let spec =
                launch_spec(&project, &service, &settings, &env(&[("PATH", &base_path)])).unwrap();
            crate::env::get(&spec.env, "PATH").unwrap().to_owned()
        };
        let custom = OsString::from(format!("/opt/node/bin:{base_path}"));
        let untouched = OsString::from(&base_path);
        for command in [
            "node x.js",
            "bun dev",
            "pnpm dev",
            "/usr/local/bin/node x.js",
            // Not in PATH, so it may be in the Runtime Path.
            "vite --port 3000",
            "./node_modules/.bin/vite",
            "./server.js",
            "./tsx watch src",
            "./dev.sh",
        ] {
            assert_eq!(path_of(command, false), custom, "{command}");
        }
        for command in [
            "'/bin/ls' -l",
            "tool --no-sandbox",
            "./bin/tool",
            "./serve.py",
            "python3 -m http.server",
        ] {
            assert_eq!(path_of(command, false), untouched, "{command}");
        }
        assert_eq!(path_of("tool | tee log && npm run dev", true), custom);
    }

    #[test]
    fn runs_in_the_service_folder() {
        let mut s = service("bun run dev");
        s.folder = Some("apps/web".into());
        let spec = launch_spec(&project(), &s, &Settings::default(), &[]).unwrap();
        assert_eq!(spec.cwd, PathBuf::from("/code/api/apps/web"));
    }

    #[test]
    fn environment_gets_colors_and_service_variables_but_no_dyld() {
        let mut s = service("node x.js");
        s.env.insert("PORT".into(), "4000".into());
        let spec = launch_spec(
            &project(),
            &s,
            &Settings::default(),
            &env(&[
                ("PATH", "/usr/bin"),
                ("DYLD_INSERT_LIBRARIES", "x"),
                ("PORT", "1"),
            ]),
        )
        .unwrap();
        let get = |k: &str| {
            spec.env
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(get("PATH"), Some("/usr/bin".into()));
        assert_eq!(get("FORCE_COLOR"), Some("1".into()));
        assert_eq!(get("PORT"), Some("4000".into()));
        assert_eq!(get("DYLD_INSERT_LIBRARIES"), None);
    }
}
