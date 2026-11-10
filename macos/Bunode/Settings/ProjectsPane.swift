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
                    Label(issue.message, systemImage: "exclamationmark.triangle.fill")
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
                        ProjectRow(model: model, project: project, toaster: toaster)
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
    let model: AppModel
    let project: ProjectState
    let toaster: Toaster
    @State private var isRemoving = false
    @State private var isNaming = false
    @State private var nameDraft = ""
    @FocusState private var isNameFocused: Bool

    var body: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 5) {
                    name
                    if project.severalLockfiles {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundStyle(.orange)
                            .help("Several lockfiles found. Pick a package manager.")
                            .accessibilityLabel("Several lockfiles found. Pick a package manager.")
                    }
                    if project.isMissing {
                        Image(systemName: "questionmark.folder")
                            .foregroundStyle(.orange)
                            .help("\(project.path.abbreviatingWithTilde) is missing.")
                            .accessibilityLabel("\(project.path.abbreviatingWithTilde) is missing.")
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
            .accessibilityElement(children: isNaming ? .contain : .combine)
            Spacer(minLength: 0)
            // A manual choice clears `severalLockfiles`; the Picker stays so
            // it can be changed or set back to Auto.
            if project.severalLockfiles || project.packageManagerChoice != nil {
                Picker("Package manager", selection: Binding(
                    get: { project.packageManagerChoice },
                    set: { model.setPackageManager(project.id, $0) }
                )) {
                    Text("Auto (\(project.detectedPackageManager.displayName))").tag(PackageManager?.none)
                    Divider()
                    ForEach([PackageManager.npm, .pnpm, .yarn, .bun], id: \.self) { manager in
                        Text(verbatim: manager.displayName).tag(PackageManager?.some(manager))
                    }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .controlSize(.small)
                .fixedSize()
            }
            Button("Remove…", action: confirmRemove)
                .controlSize(.small)
                .disabled(isRemoving)
        }
        .frame(minHeight: 36)
        .task(id: model.projectAwaitingName) {
            guard model.projectAwaitingName == project.id else { return }
            model.projectAwaitingName = nil
            startNaming()
        }
    }

    /// Click to type a name. Left blank, the folder's name stays.
    @ViewBuilder
    private var name: some View {
        if isNaming {
            TextField("Project name", text: $nameDraft, prompt: Text(verbatim: project.folderName))
                .labelsHidden()
                .textFieldStyle(.roundedBorder)
                .controlSize(.small)
                .frame(maxWidth: 220)
                .focused($isNameFocused)
                .task { isNameFocused = true }
                .onSubmit(saveName)
                .onExitCommand { isNaming = false }
                .onChange(of: isNameFocused) { _, focused in
                    if !focused { saveName() }
                }
                .onDisappear(perform: saveName)
        } else {
            Button(action: startNaming) {
                Text(project.name)
                    .lineLimit(1)
            }
            .buttonStyle(.plain)
            .help("Click to rename")
        }
    }

    private func startNaming() {
        nameDraft = project.name == project.folderName ? "" : project.name
        isNaming = true
    }

    private func saveName() {
        guard isNaming else { return }
        isNaming = false
        model.setProjectAlias(project.id, nameDraft)
    }

    /// The Remove alert (design-handoff §2.4). Cancel is the default button:
    /// removing is the destructive choice.
    private func confirmRemove() {
        let running = model.runningCount(in: project)
        let alert = NSAlert()
        alert.messageText = String(localized: "Remove “\(project.name)”?")
        alert.informativeText = AppModel.removalMessage(for: project, running: running)
        alert.addButton(withTitle: String(localized: "Cancel"))
        let remove = alert.addButton(withTitle: running > 0
            ? String(localized: "Stop \(running) and Remove")
            : String(localized: "Remove"))
        remove.hasDestructiveAction = true
        let respond = { (response: NSApplication.ModalResponse) in
            guard response == .alertSecondButtonReturn else { return }
            isRemoving = true
            Task {
                toaster.show(await model.removeProject(project.id))
                isRemoving = false
            }
        }
        if let window = NSApp.keyWindow {
            alert.beginSheetModal(for: window, completionHandler: respond)
        } else {
            respond(alert.runModal())
        }
    }
}
