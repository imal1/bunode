import SwiftUI

/// New Service (design-handoff §2.2, §5.2): pick a Project, then a
/// `package.json` script or type a command. The name follows from it.
/// Nothing starts.
struct AddServiceView: View {
    let model: AppModel
    let router: PopoverRouter
    let toaster: Toaster

    @State private var projectID: UInt64
    @State private var scripts: ProjectScripts?
    @State private var selection: ScriptKey?
    @State private var command = ""
    @State private var error: String?
    @State private var isAdding = false
    @FocusState private var commandFocused: Bool

    init(model: AppModel, projectID: UInt64, router: PopoverRouter, toaster: Toaster) {
        self.model = model
        self.router = router
        self.toaster = toaster
        _projectID = State(initialValue: projectID)
    }

    /// A script is identified by its folder and its name.
    struct ScriptKey: Hashable {
        let folder: String?
        let name: String
    }

    private var project: ProjectState? { model.project(projectID) }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let project {
                TopBar(title: String(localized: "New Service"), onBack: router.back) {
                    Text("Runs from \(Text(project.runPath(folder: folder).abbreviatingWithTilde).monospaced())")
                        .font(.system(size: 11))
                        .truncationMode(.middle)
                }
                PopoverScroll {
                    VStack(alignment: .leading, spacing: 10) {
                        pickerCard
                        fieldsCard(project)
                    }
                }
                buttons(project)
            }
        }
        .task(id: projectID) { await load() }
        .onChange(of: command) { _, newValue in
            // Editing the command by hand unselects the script.
            if let option = selectedOption, option.command != newValue {
                selection = nil
            }
            error = nil
        }
    }

    // MARK: - Cards

    private var pickerCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            pickerLabel("Project")
            FlowLayout(spacing: 6) {
                ForEach(model.snapshot.projects, id: \.id) { project in
                    Chip(title: project.name, font: .system(size: 12), isSelected: project.id == projectID) {
                        projectID = project.id
                    }
                }
            }
            if let scripts, scripts.hasPackageJson, !allScripts.isEmpty {
                pickerLabel("Scripts in package.json")
                    .padding(.top, 2)
                scriptGroups(scripts)
            }
        }
        .padding(EdgeInsets(top: 10, leading: 12, bottom: 10, trailing: 12))
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.background.secondary, in: RoundedRectangle(cornerRadius: 10))
        .padding(.horizontal, 2)
    }

    private func pickerLabel(_ title: LocalizedStringKey) -> some View {
        Text(title)
            .font(.system(size: 11))
            .foregroundStyle(.secondary)
    }

    @ViewBuilder
    private func scriptGroups(_ scripts: ProjectScripts) -> some View {
        ForEach(scripts.groups.filter { !$0.scripts.isEmpty }, id: \.folder) { group in
            if let package = group.packageName {
                // A workspace package: its name, and where its scripts run.
                HStack(spacing: 4) {
                    Text(package)
                        .font(.caption.weight(.semibold))
                    if let folder = group.folder {
                        Text(folder)
                            .font(.caption.monospaced())
                    }
                }
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
                .padding(.top, 2)
            }
            FlowLayout(spacing: 6) {
                ForEach(group.scripts, id: \.name) { option in
                    let key = ScriptKey(folder: group.folder, name: option.name)
                    Chip(
                        title: option.name,
                        font: .system(size: 11, design: .monospaced),
                        isSelected: selection == key
                    ) {
                        select(option, key: key)
                    }
                    .disabled(option.used)
                    .opacity(option.used ? 0.4 : 1)
                    .help(option.used ? String(localized: "Already a Service: \(option.command)") : option.command)
                }
            }
        }
    }

    private func fieldsCard(_ project: ProjectState) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Card {
                CardRow(title: "Command") {
                    TextField("Command", text: $command, prompt: Text(verbatim: "bun run dev"))
                        .labelsHidden()
                        .font(.callout.monospaced())
                        .frame(width: 200)
                        .focused($commandFocused)
                }
            }
            .controlSize(.small)
            if let message = nameProblem(in: project) ?? error {
                Text(message)
                    .font(.system(size: 11))
                    .foregroundStyle(Color(nsColor: .systemRed))
                    .padding(.horizontal, 14)
            }
        }
    }

    private func buttons(_ project: ProjectState) -> some View {
        HStack(spacing: 6) {
            Spacer()
            Button("Cancel", action: router.back)
                .keyboardShortcut(.cancelAction)
            Button("Add") { add(to: project) }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(!canAdd(to: project))
        }
        .controlSize(.small)
        .padding(EdgeInsets(top: 10, leading: 2, bottom: 0, trailing: 2))
    }

    // MARK: - Logic

    /// The name a new Service gets (design-handoff §6.2): its script's, or
    /// the last word of a hand-typed command, cut to the 40 characters the
    /// Supervisor accepts. It counts Unicode scalars, so this does too.
    static func serviceName(script: String?, command: String) -> String {
        let name = script ?? command.split(whereSeparator: \.isWhitespace).last.map(String.init) ?? ""
        return String(String.UnicodeScalarView(name.unicodeScalars.prefix(40)))
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var name: String {
        Self.serviceName(script: selectedOption?.name, command: command)
    }

    private var allScripts: [(key: ScriptKey, option: ScriptOption)] {
        (scripts?.groups ?? []).flatMap { group in
            group.scripts.map { (ScriptKey(folder: group.folder, name: $0.name), $0) }
        }
    }

    private var selectedOption: ScriptOption? {
        allScripts.first { $0.key == selection }?.option
    }

    private func load() async {
        let loaded = await model.projectScripts(projectID)
        // Switched Project in the meantime.
        guard !Task.isCancelled else { return }
        scripts = loaded
        selection = nil
        command = ""
        if let first = allScripts.first(where: { !$0.option.used }) {
            select(first.option, key: first.key)
        } else {
            commandFocused = true
        }
    }

    private func select(_ option: ScriptOption, key: ScriptKey) {
        selection = key
        command = option.command
    }

    /// The folder the new Service runs in: the selected script's package.
    private var folder: String? {
        selectedOption == nil ? nil : selection?.folder
    }

    /// Names are unique among the Services of the same folder.
    private func nameProblem(in project: ProjectState) -> String? {
        let taken = project.services.contains {
            $0.folder == folder && $0.name.lowercased() == name.lowercased()
        }
        return taken ? String(localized: "Name already used in \(project.name)") : nil
    }

    private func canAdd(to project: ProjectState) -> Bool {
        !isAdding
            && !command.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && nameProblem(in: project) == nil
    }

    private func add(to project: ProjectState) {
        guard canAdd(to: project) else { return }
        isAdding = true
        let folder = self.folder
        Task {
            let result = await model.addService(to: project.id, name: name, command: command, folder: folder)
            isAdding = false
            switch result {
            case .success(let service):
                toaster.show(String(localized: "Added \(project.name) › \(service.displayName)"))
                router.popToHome()
            case .failure(let failure):
                error = failure.userMessage
            }
        }
    }
}

/// A pill for a Project or a `package.json` script (design-handoff §2.2).
private struct Chip: View {
    let title: String
    let font: Font
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(font)
                .lineLimit(1)
                .truncationMode(.middle)
                .padding(.horizontal, 9)
                .frame(height: 24)
                .frame(maxWidth: 290)
                .foregroundStyle(isSelected ? Color.white : Color.primary)
                .background(
                    isSelected ? Color.accentColor : Color(nsColor: .controlBackgroundColor),
                    in: Capsule()
                )
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}
