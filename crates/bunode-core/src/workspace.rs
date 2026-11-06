//! Finds the packages of a monorepo: the `workspaces` field of
//! `package.json` (npm, yarn, Bun) or `pnpm-workspace.yaml` (pnpm).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::package::{self, Script};

/// A workspace package: a folder of the Project with its own `package.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The `name` of its `package.json`, else its folder's name.
    pub name: String,
    /// Relative to the Project's directory, with `/` separators.
    pub folder: String,
    pub scripts: Vec<Script>,
}

/// Folders never searched for packages.
const SKIPPED: &[&str] = &["node_modules", ".git"];

/// The workspace packages of the Project at `root`, sorted by folder.
/// A Project that is not a monorepo has none.
pub fn packages(root: &Path) -> Vec<Package> {
    let patterns = patterns(root);
    let (exclude, include): (Vec<&String>, Vec<&String>) =
        patterns.iter().partition(|p| p.starts_with('!'));
    let exclude: Vec<Vec<&str>> = exclude
        .iter()
        .map(|p| segments(p.trim_start_matches('!')))
        .collect();

    let mut folders: Vec<String> = Vec::new();
    for pattern in include {
        for folder in expand(root, &segments(pattern)) {
            let parts: Vec<&str> = folder.split('/').collect();
            let excluded = exclude.iter().any(|ex| matches_path(ex, &parts));
            if !excluded && !folder.is_empty() && !folders.contains(&folder) {
                folders.push(folder);
            }
        }
    }
    folders.sort();

    folders
        .into_iter()
        .filter_map(|folder| {
            let dir = root.join(&folder);
            let text = fs::read_to_string(dir.join("package.json")).ok()?;
            let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| folder.rsplit('/').next().unwrap_or(&folder).to_owned());
            Some(Package {
                name,
                scripts: package::scripts(&dir).unwrap_or_default(),
                folder,
            })
        })
        .collect()
}

/// The package globs the Project declares.
fn patterns(root: &Path) -> Vec<String> {
    if let Ok(yaml) = fs::read_to_string(root.join("pnpm-workspace.yaml")) {
        return pnpm_packages(&yaml);
    }
    let Some(value) = fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return Vec::new();
    };
    let list = match value.get("workspaces") {
        Some(Value::Array(list)) => list.clone(),
        // yarn's `{ "packages": [...], "nohoist": [...] }`
        Some(Value::Object(object)) => object
            .get("packages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    list.iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// Reads the `packages:` list of a `pnpm-workspace.yaml`. Only that list's
/// simple form is understood, which is what pnpm documents.
fn pnpm_packages(yaml: &str) -> Vec<String> {
    let mut in_packages = false;
    let mut result = Vec::new();
    for line in yaml.lines() {
        let without_comment = line.split(" #").next().unwrap_or("");
        if without_comment.trim().is_empty() || without_comment.trim_start().starts_with('#') {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if !indented {
            in_packages = without_comment.trim() == "packages:";
            continue;
        }
        if in_packages && let Some(item) = without_comment.trim().strip_prefix('-') {
            let item = item.trim().trim_matches(|c| c == '\'' || c == '"');
            if !item.is_empty() {
                result.push(item.to_owned());
            }
        }
    }
    result
}

fn segments(pattern: &str) -> Vec<&str> {
    pattern
        .trim_start_matches("./")
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect()
}

/// The folders under `root` that match a glob, as relative paths.
fn expand(root: &Path, pattern: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    walk(root, PathBuf::new(), pattern, &mut found);
    found
}

fn walk(root: &Path, relative: PathBuf, pattern: &[&str], found: &mut Vec<String>) {
    let Some((first, rest)) = pattern.split_first() else {
        found.push(relative.to_string_lossy().replace('\\', "/"));
        return;
    };
    if *first == "**" {
        // Zero folders, or one more and still `**`.
        walk(root, relative.clone(), rest, found);
        for child in subfolders(&root.join(&relative)) {
            walk(root, relative.join(&child), pattern, found);
        }
        return;
    }
    if !first.contains('*') {
        if root.join(&relative).join(first).is_dir() {
            walk(root, relative.join(first), rest, found);
        }
        return;
    }
    for child in subfolders(&root.join(&relative)) {
        if wildcard(first, &child) {
            walk(root, relative.join(&child), rest, found);
        }
    }
}

fn subfolders(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !SKIPPED.contains(&name.as_str()))
        .collect();
    names.sort();
    names
}

/// Whether a relative path matches a glob's segments.
fn matches_path(pattern: &[&str], path: &[&str]) -> bool {
    match (pattern.split_first(), path.split_first()) {
        (None, None) => true,
        (Some((&"**", rest)), _) => {
            matches_path(rest, path) || (!path.is_empty() && matches_path(pattern, &path[1..]))
        }
        (Some((first, rest)), Some((name, tail))) => {
            wildcard(first, name) && matches_path(rest, tail)
        }
        _ => false,
    }
}

/// `*` matches any run of characters within one folder name.
fn wildcard(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let mut rest = name;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            match rest.strip_prefix(part) {
                Some(after) => rest = after,
                None => return false,
            }
        } else if i == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            match rest.find(part) {
                Some(at) => rest = &rest[at + part.len()..],
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, contents) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        dir
    }

    fn folders(root: &Path) -> Vec<String> {
        packages(root).into_iter().map(|p| p.folder).collect()
    }

    #[test]
    fn npm_and_bun_workspaces_array() {
        let dir = tree(&[
            (
                "package.json",
                r#"{"workspaces": ["apps/*", "packages/ui"]}"#,
            ),
            (
                "apps/web/package.json",
                r#"{"name": "@acme/web", "scripts": {"dev": "vite"}}"#,
            ),
            (
                "apps/docs/package.json",
                r#"{"scripts": {"dev": "astro dev"}}"#,
            ),
            ("apps/notes/README.md", ""),
            ("packages/ui/package.json", r#"{"name": "ui"}"#),
            ("packages/other/package.json", "{}"),
        ]);
        let found = packages(dir.path());
        assert_eq!(
            found.iter().map(|p| p.folder.as_str()).collect::<Vec<_>>(),
            ["apps/docs", "apps/web", "packages/ui"]
        );
        assert_eq!(found[0].name, "docs");
        assert_eq!(found[1].name, "@acme/web");
        assert_eq!(found[1].scripts[0].name, "dev");
    }

    #[test]
    fn yarn_workspaces_object() {
        let dir = tree(&[
            (
                "package.json",
                r#"{"workspaces": {"packages": ["packages/*"], "nohoist": ["**/x"]}}"#,
            ),
            ("packages/a/package.json", "{}"),
        ]);
        assert_eq!(folders(dir.path()), ["packages/a"]);
    }

    #[test]
    fn pnpm_workspace_yaml_with_exclusions() {
        let dir = tree(&[
            ("package.json", "{}"),
            (
                "pnpm-workspace.yaml",
                "# monorepo\npackages:\n  - 'apps/*'\n  - \"tools/**\"\n  - '!**/test/**'\ncatalog:\n  react: ^19\n",
            ),
            ("apps/web/package.json", "{}"),
            ("tools/cli/package.json", "{}"),
            ("tools/cli/test/fixture/package.json", "{}"),
            ("tools/deep/nested/package.json", "{}"),
            ("apps/web/node_modules/dep/package.json", "{}"),
        ]);
        assert_eq!(
            folders(dir.path()),
            ["apps/web", "tools/cli", "tools/deep/nested"]
        );
    }

    #[test]
    fn a_plain_project_has_no_packages() {
        let dir = tree(&[("package.json", r#"{"scripts": {"dev": "vite"}}"#)]);
        assert!(packages(dir.path()).is_empty());
        assert!(packages(tree(&[]).path()).is_empty());
    }

    #[test]
    fn wildcards_within_a_name() {
        assert!(wildcard("app-*", "app-web"));
        assert!(!wildcard("app-*", "lib-web"));
        assert!(wildcard("*-svc", "api-svc"));
        assert!(wildcard("a*c*e", "abcde"));
        assert!(!wildcard("a*c*e", "abcd"));
    }
}
