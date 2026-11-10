//! The configuration the user builds up: Projects, their Services, and
//! global settings. This is what gets persisted (see `store`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub type ProjectId = u64;
pub type ServiceId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    /// The name as the UI shows it: "npm", "pnpm", "yarn", "Bun".
    pub fn display_name(self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "Bun",
        }
    }
}

/// A working directory registered in Bunode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ProjectId,
    /// The folder's name, given when the Project was added.
    pub name: String,
    /// The short name the user gave it in Settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub path: PathBuf,
    /// The package manager the user picked. `None` means detect it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<PackageManager>,
    #[serde(default)]
    pub services: Vec<Service>,
}

impl Project {
    /// What Bunode calls it: the alias, or else the folder's name.
    pub fn display_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

/// A named run configuration of a Project: the unit the user starts and stops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: ServiceId,
    pub name: String,
    pub command: String,
    /// Where the command runs, relative to the Project's directory.
    /// `None` is the Project's directory itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
    /// The port the Service is expected to listen on, checked before each start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Run the command through the shell (pipes, `&&`, globs) instead of
    /// executing it directly.
    #[serde(default)]
    pub run_through_shell: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// How many Runs this Service has had; the next Run is `runs + 1`.
    #[serde(default)]
    pub runs: u64,
}

/// Global preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// General › Show running count.
    pub show_running_count: bool,
    /// Advanced › Force stop after, in seconds.
    pub force_stop_after_secs: u32,
    /// Advanced › Log lines kept, per Service.
    pub log_lines_kept: u32,
    /// Advanced › Node.js path override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_path: Option<PathBuf>,
    /// Advanced › Bun path override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bun_path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_running_count: true,
            force_stop_after_secs: 5,
            log_lines_kept: 10_000,
            node_path: None,
            bun_path: None,
        }
    }
}

/// Everything that is saved to `config.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    /// The next id to hand out to a Project or a Service.
    pub next_id: u64,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub settings: Settings,
}

impl Config {
    pub const VERSION: u32 = 1;

    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn project_mut(&mut self, id: ProjectId) -> Option<&mut Project> {
        self.projects.iter_mut().find(|p| p.id == id)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: Config::VERSION,
            next_id: 1,
            projects: Vec::new(),
            settings: Settings::default(),
        }
    }
}
