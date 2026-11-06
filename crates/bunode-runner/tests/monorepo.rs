//! Monorepo packages' scripts as Services (requirements §3.1).

mod support;

use bunode_core::{AddProjectOutcome, LogStream, NewService, PackageManager, RunStatus};
use support::*;

fn project(h: &Harness, fixture_name: &str) -> bunode_core::ProjectState {
    match h.supervisor.add_project(&fixture(fixture_name)).unwrap() {
        AddProjectOutcome::Added { project } | AddProjectOutcome::AlreadyAdded { project } => {
            project
        }
    }
}

#[test]
fn finds_the_packages_of_every_kind_of_workspace() {
    let h = Harness::new();
    for (name, manager) in [
        ("monorepo-npm", PackageManager::Npm),
        ("monorepo-yarn", PackageManager::Yarn),
        ("monorepo-pnpm", PackageManager::Pnpm),
        ("monorepo-bun", PackageManager::Bun),
    ] {
        let project = project(&h, name);
        assert_eq!(project.package_manager, manager, "{name}");
        let scripts = h.supervisor.project_scripts(project.id).unwrap();
        let folders: Vec<Option<&str>> =
            scripts.groups.iter().map(|g| g.folder.as_deref()).collect();
        assert_eq!(
            folders,
            [None, Some("apps/web"), Some("packages/ui")],
            "{name}"
        );
        let web = &scripts.groups[1];
        assert_eq!(
            web.package_name.as_deref(),
            Some(format!("@{name}/web").as_str())
        );
        assert_eq!(web.scripts[0].name, "where");
    }
}

#[test]
fn package_services_run_in_their_folder() {
    for (name, tool) in [
        ("monorepo-npm", "npm"),
        ("monorepo-pnpm", "pnpm"),
        ("monorepo-bun", "bun"),
    ] {
        require_tool(tool);
        let h = Harness::new();
        let project = project(&h, name);
        let scripts = h.supervisor.project_scripts(project.id).unwrap();
        let web = &scripts.groups[1];
        let service = h
            .supervisor
            .add_service(
                project.id,
                NewService {
                    name: web.scripts[0].name.clone(),
                    command: web.scripts[0].command.clone(),
                    folder: web.folder.clone(),
                },
            )
            .unwrap();
        assert_eq!(service.display_name, "web › where");

        // A `where` in another folder is fine; a second one in apps/web is not.
        assert!(
            h.supervisor
                .add_service(
                    project.id,
                    NewService {
                        name: "where".into(),
                        command: "x".into(),
                        folder: None,
                    }
                )
                .is_ok()
        );
        assert!(
            h.supervisor
                .add_service(
                    project.id,
                    NewService {
                        name: "WHERE".into(),
                        command: "x".into(),
                        folder: Some("apps/web".into()),
                    }
                )
                .is_err()
        );

        h.supervisor.start(service.id).unwrap();
        h.wait(service.id, &format!("{name}: the script"), |s| {
            s.run_number.is_some() && !s.status.is_live()
        });
        let state = h.state(service.id);
        assert_eq!(state.status, RunStatus::Stopped, "{name}: {state:?}");
        let cwd = h
            .supervisor
            .log_lines(service.id, 0, 100)
            .into_iter()
            .filter(|l| l.stream == LogStream::Stdout)
            .find_map(|l| l.text.strip_prefix("cwd ").map(str::to_owned))
            .unwrap_or_default();
        assert!(cwd.ends_with(&format!("{name}/apps/web")), "{name}: {cwd}");
    }
}
