//! Turns a Service's configuration into what `bunode-runner` executes.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use crate::model::{Project, Service, Settings};

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
/// start from. Fails with a message for the user when the command can't be
/// run.
pub fn launch_spec(
    project: &Project,
    service: &Service,
    settings: &Settings,
    base_env: &[(OsString, OsString)],
) -> Result<LaunchSpec, String> {
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
        let mut words = parse_command(&service.command)?;
        let program = words.remove(0);
        (program, words)
    };

    let cwd = match &service.folder {
        Some(folder) => project.path.join(folder),
        None => project.path.clone(),
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
pub fn parse_command(command: &str) -> Result<Vec<String>, String> {
    let words = shell_words::split(command)
        .map_err(|_| "The command has an unmatched quote.".to_owned())?;
    if words.is_empty() {
        return Err("The command is empty.".into());
    }
    Ok(words)
}

pub(crate) fn set_env(env: &mut Vec<(OsString, OsString)>, key: OsString, value: OsString) {
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
        assert!(launch_spec(&project(), &service("node 'oops"), &settings, &[]).is_err());
        assert!(launch_spec(&project(), &service("   "), &settings, &[]).is_err());
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
