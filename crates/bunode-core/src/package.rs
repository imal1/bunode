//! Reading a Project's `package.json` and working out its package manager.

use std::fs;
use std::path::Path;

use serde_json::Value;

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

/// A script from the `scripts` field of `package.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub name: String,
    /// What the script runs, as written in `package.json`.
    pub body: String,
}

/// The parts of `package.json` Bunode reads. Fields with an unexpected
/// shape are ignored rather than failing the whole file.
#[derive(Debug, Default)]
pub(crate) struct PackageJson {
    pub package_manager: Option<String>,
    /// In the order `package.json` lists them.
    pub scripts: Vec<Script>,
}

impl PackageJson {
    fn from_value(value: &Value) -> PackageJson {
        let package_manager = value
            .get("packageManager")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let scripts = value
            .get("scripts")
            .and_then(Value::as_object)
            .map(|scripts| {
                scripts
                    .iter()
                    .filter_map(|(name, body)| {
                        Some(Script {
                            name: name.clone(),
                            body: body.as_str()?.to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        PackageJson {
            package_manager,
            scripts,
        }
    }
}

pub(crate) fn read_package_json(dir: &Path) -> Option<PackageJson> {
    let text = fs::read_to_string(dir.join("package.json")).ok()?;
    // A package.json that does not parse still counts as present.
    let value = serde_json::from_str(&text).unwrap_or(Value::Null);
    Some(PackageJson::from_value(&value))
}

/// The scripts of `dir/package.json`, or `None` when there is no
/// `package.json`.
pub fn scripts(dir: &Path) -> Option<Vec<Script>> {
    read_package_json(dir).map(|p| p.scripts)
}

/// The command that runs script `name` with `manager` (design-handoff §5.2).
pub fn script_command(manager: PackageManager, name: &str) -> String {
    let name = shell_quote(name);
    match manager {
        PackageManager::Bun => format!("bun run {name}"),
        PackageManager::Pnpm => format!("pnpm {name}"),
        PackageManager::Npm => format!("npm run {name}"),
        PackageManager::Yarn => format!("yarn {name}"),
    }
}

/// Quotes `word` for a POSIX shell, unless it needs no quoting.
pub fn shell_quote(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
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
    fn scripts_keep_their_order_and_skip_odd_entries() {
        let dir = project(&[(
            "package.json",
            r#"{"scripts": {"dev": "vite", "build": "vite build", "weird": 3, "storybook": "storybook dev"}}"#,
        )]);
        let names: Vec<String> = scripts(dir.path())
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, ["dev", "build", "storybook"]);
        assert_eq!(scripts(project(&[]).path()), None);
        assert_eq!(
            scripts(project(&[("package.json", "{")]).path()),
            Some(vec![])
        );
    }

    #[test]
    fn script_commands_follow_the_package_manager() {
        assert_eq!(script_command(PackageManager::Bun, "dev"), "bun run dev");
        assert_eq!(script_command(PackageManager::Pnpm, "dev"), "pnpm dev");
        assert_eq!(script_command(PackageManager::Npm, "dev"), "npm run dev");
        assert_eq!(script_command(PackageManager::Yarn, "dev"), "yarn dev");
        assert_eq!(
            script_command(PackageManager::Npm, "build:web"),
            "npm run build:web"
        );
        assert_eq!(
            script_command(PackageManager::Npm, "my script"),
            "npm run 'my script'"
        );
        assert_eq!(
            script_command(PackageManager::Npm, "it's"),
            r"npm run 'it'\''s'"
        );
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
