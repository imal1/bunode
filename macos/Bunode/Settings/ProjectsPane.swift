import AppKit
import SwiftUI

/// Settings › Projects (design-handoff §2.3): the registered Projects, and
/// `Add Project…`. Services are set up from the menu bar, not here.
struct ProjectsPane: View {
    let model: AppModel
    @Bindable var navigation: SettingsNavigation
    let toaster: Toaster

    var body: some View {
        Form {
            if let issue = model.snapshot.configIssue {
                Section {
                    Label(issue, systemImage: "exclamationmark.triangle.fill")
                        .font(.callout)
                        .symbolRenderingMode(.multicolor)
                }
            }
            Section {
                if model.snapshot.projects.isEmpty {
                    Text("No Projects yet.")
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, minHeight: 36)
                } else {
                    ForEach(model.snapshot.projects, id: \.id) { project in
                        ProjectRow(project: project)
                    }
                }
            } footer: {
                HStack {
                    Text("Services are added and set up from the menu bar.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Button("Add Project…", action: addProject)
                }
            }
        }
        .formStyle(.grouped)
        .task(id: navigation.pendingAddProject) {
            if navigation.pendingAddProject {
                navigation.pendingAddProject = false
                addProject()
            }
        }
    }

    private func addProject() {
        Task {
            let window = NSApp.keyWindow
            guard let url = await Panels.chooseFolder(prompt: String(localized: "Add Project"), in: window) else { return }
            toaster.show(await model.addProject(at: url))
        }
    }
}

private struct ProjectRow: View {
    let project: ProjectState

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 5) {
                Text(project.name)
                if project.severalLockfiles {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                        .help("Several lockfiles found. Pick a package manager.")
                        .accessibilityLabel("Several lockfiles found. Pick a package manager.")
                }
            }
            HStack(spacing: 0) {
                Text(project.path.abbreviatingWithTilde)
                    .font(.caption.monospaced())
                    .truncationMode(.middle)
                    .layoutPriority(-1)
                Text(verbatim: " · \(project.packageManager.displayName) · ")
                    .font(.caption)
                    .fixedSize()
                Text("\(project.services.count) Services")
                    .font(.caption)
                    .fixedSize()
            }
            .lineLimit(1)
            .foregroundStyle(.secondary)
            .help(project.path)
        }
        .frame(minHeight: 36, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}
