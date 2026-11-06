//! Reading a Project's `package.json` and working out its package manager.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::model::PackageManager;

/// What Bunode found out about a Project directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// The package manager to use when the user has not picked one.
    pub manager: PackageManager,
    pub source: DetectionSource,
    /// Lockfiles of more than one package manager were found, and nothing
    /// else said which one to use.
    pub several_lockfiles: bool,
    pub has_package_json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionSource {
    /// The `packageManager` field of `package.json`.
    PackageManagerField,
    Lockfile,
    /// Nothing pointed at a package manager, so npm.
    Default,
}

/// Lockfiles in the order they win (requirements §3.1).
const LOCKFILES: &[(&str, PackageManager)] = &[
    ("bun.lock", PackageManager::Bun),
    ("bun.lockb", PackageManager::Bun),
    ("pnpm-lock.yaml", PackageManager::Pnpm),
    ("yarn.lock", PackageManager::Yarn),
    ("package-lock.json", PackageManager::Npm),
];

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageJson {
    #[serde(default)]
    pub package_manager: Option<String>,
}

pub(crate) fn read_package_json(dir: &Path) -> Option<PackageJson> {
    let text = fs::read_to_string(dir.join("package.json")).ok()?;
    // A package.json that does not parse still counts as present.
    Some(serde_json::from_str(&text).unwrap_or_default())
}

/// Detects the package manager of `dir`: the `packageManager` field of
/// `package.json`, then the lockfiles, then npm.
pub fn detect(dir: &Path) -> Detection {
    let package_json = read_package_json(dir);
    let has_package_json = package_json.is_some();

    if let Some(manager) = package_json
        .as_ref()
        .and_then(|p| p.package_manager.as_deref())
        .and_then(parse_package_manager_field)
    {
        return Detection {
            manager,
            source: DetectionSource::PackageManagerField,
            several_lockfiles: false,
            has_package_json,
        };
    }

    let mut found: Vec<PackageManager> = Vec::new();
    for (file, manager) in LOCKFILES {
        if dir.join(file).is_file() && !found.contains(manager) {
            found.push(*manager);
        }
    }

    match found.first() {
        Some(&manager) => Detection {
            manager,
            source: DetectionSource::Lockfile,
            several_lockfiles: found.len() > 1,
            has_package_json,
        },
        None => Detection {
            manager: PackageManager::Npm,
            source: DetectionSource::Default,
            several_lockfiles: false,
            has_package_json,
        },
    }
}

/// Parses `"pnpm@9.1.0+sha512…"` into pnpm. Unknown names give `None`.
fn parse_package_manager_field(value: &str) -> Option<PackageManager> {
    let name = value.split('@').next()?.trim();
    match name {
        "npm" => Some(PackageManager::Npm),
        "pnpm" => Some(PackageManager::Pnpm),
        "yarn" => Some(PackageManager::Yarn),
        "bun" => Some(PackageManager::Bun),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn project(files: &[(&str, &str)]) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, contents) in files {
            fs::write(dir.path().join(name), contents).unwrap();
        }
        dir
    }

    fn detected(files: &[(&str, &str)]) -> Detection {
        detect(project(files).path())
    }

    #[test]
    fn package_manager_field_wins_over_lockfiles() {
        let d = detected(&[
            (
                "package.json",
                r#"{"packageManager": "pnpm@9.1.0+sha512.abc"}"#,
            ),
            ("bun.lock", ""),
            ("yarn.lock", ""),
        ]);
        assert_eq!(d.manager, PackageManager::Pnpm);
        assert_eq!(d.source, DetectionSource::PackageManagerField);
        assert!(!d.several_lockfiles);
    }

    #[test]
    fn each_lockfile_maps_to_its_manager() {
        for (file, manager) in LOCKFILES {
            let d = detected(&[("package.json", "{}"), (file, "")]);
            assert_eq!(d.manager, *manager, "{file}");
            assert_eq!(d.source, DetectionSource::Lockfile);
            assert!(!d.several_lockfiles, "{file}");
        }
    }

    #[test]
    fn lockfiles_follow_the_documented_order() {
        let d = detected(&[
            ("pnpm-lock.yaml", ""),
            ("yarn.lock", ""),
            ("package-lock.json", ""),
        ]);
        assert_eq!(d.manager, PackageManager::Pnpm);
        let d = detected(&[("yarn.lock", ""), ("package-lock.json", "")]);
        assert_eq!(d.manager, PackageManager::Yarn);
        let d = detected(&[("bun.lockb", ""), ("package-lock.json", "")]);
        assert_eq!(d.manager, PackageManager::Bun);
    }

    #[test]
    fn several_lockfiles_are_flagged() {
        let d = detected(&[("package-lock.json", ""), ("bun.lock", "")]);
        assert_eq!(d.manager, PackageManager::Bun);
        assert!(d.several_lockfiles);
    }

    #[test]
    fn both_bun_lockfiles_are_not_a_conflict() {
        let d = detected(&[("bun.lock", ""), ("bun.lockb", "")]);
        assert_eq!(d.manager, PackageManager::Bun);
        assert!(!d.several_lockfiles);
    }

    #[test]
    fn defaults_to_npm() {
        let d = detected(&[("package.json", "{}")]);
        assert_eq!(d.manager, PackageManager::Npm);
        assert_eq!(d.source, DetectionSource::Default);
        assert!(d.has_package_json);

        let d = detected(&[]);
        assert_eq!(d.manager, PackageManager::Npm);
        assert!(!d.has_package_json);
    }

    #[test]
    fn unknown_or_broken_package_manager_field_falls_through() {
        let d = detected(&[
            ("package.json", r#"{"packageManager": "deno@2"}"#),
            ("yarn.lock", ""),
        ]);
        assert_eq!(d.manager, PackageManager::Yarn);
        let d = detected(&[("package.json", "{ not json"), ("yarn.lock", "")]);
        assert_eq!(d.manager, PackageManager::Yarn);
        assert!(d.has_package_json);
    }
}
