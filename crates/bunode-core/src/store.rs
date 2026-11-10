//! Persists the `Config` as JSON in Bunode's data directory.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::Config;
use crate::snapshot::{ConfigIssue, ConfigProblem};

pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(data_dir: &Path) -> ConfigStore {
        ConfigStore {
            path: data_dir.join("config.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the configuration. A missing file is an empty configuration. A
    /// file that cannot be read or parsed is moved aside, so that the next
    /// save does not destroy what the user may still want to recover.
    pub fn load(&self) -> (Config, Option<ConfigIssue>) {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Config::default(), None),
            Err(e) => return (Config::default(), Some(self.set_aside(unreadable(e)))),
        };
        match serde_json::from_str::<Config>(&text) {
            Ok(config) if config.version <= Config::VERSION => (config, None),
            Ok(config) => {
                let problem = ConfigProblem::NewerFormat {
                    version: config.version,
                };
                (Config::default(), Some(self.set_aside(problem)))
            }
            Err(e) => (Config::default(), Some(self.set_aside(unreadable(e)))),
        }
    }

    /// Writes the configuration atomically: a crash mid-write leaves the
    /// previous file intact.
    pub fn save(&self, config: &Config) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&tmp)?;
            serde_json::to_writer_pretty(&mut file, config)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }
        fs::rename(&tmp, &self.path)
    }

    fn set_aside(&self, problem: ConfigProblem) -> ConfigIssue {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup = self
            .path
            .with_file_name(format!("config.unreadable-{stamp}.json"));
        let moved = fs::rename(&self.path, &backup).is_ok();
        ConfigIssue {
            path: self.path.display().to_string(),
            backup: moved.then(|| backup.display().to_string()),
            problem,
        }
    }
}

fn unreadable(error: impl std::fmt::Display) -> ConfigProblem {
    ConfigProblem::Unreadable {
        detail: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PackageManager, Project};

    #[test]
    fn missing_file_is_an_empty_config() {
        let dir = tempfile::tempdir().unwrap();
        let (config, issue) = ConfigStore::new(dir.path()).load();
        assert_eq!(config, Config::default());
        assert!(issue.is_none());
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(&dir.path().join("nested"));
        let mut config = Config::default();
        let id = config.allocate_id();
        config.projects.push(Project {
            id,
            name: "api".into(),
            alias: None,
            path: "/code/api".into(),
            package_manager: Some(PackageManager::Bun),
            services: vec![],
        });
        store.save(&config).unwrap();
        let (loaded, issue) = store.load();
        assert!(issue.is_none());
        assert_eq!(loaded, config);
    }

    #[test]
    fn corrupt_file_is_set_aside_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(dir.path());
        fs::write(store.path(), "{ \"version\": 1, \"projects\": [ oops").unwrap();

        let (config, issue) = store.load();
        assert_eq!(config, Config::default());
        let issue = issue.expect("an issue");
        let backup = issue.backup.expect("a backup");
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            "{ \"version\": 1, \"projects\": [ oops"
        );
        assert!(!store.path().exists());
        assert_eq!(issue.path, store.path().display().to_string());
        assert!(matches!(issue.problem, ConfigProblem::Unreadable { .. }));
    }

    #[test]
    fn newer_format_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(dir.path());
        fs::write(store.path(), r#"{"version": 99, "nextId": 1}"#).unwrap();
        let (config, issue) = store.load();
        assert_eq!(config, Config::default());
        assert_eq!(
            issue.unwrap().problem,
            ConfigProblem::NewerFormat { version: 99 }
        );
    }
}
