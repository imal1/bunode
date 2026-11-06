import SwiftUI

/// New Service (design-handoff §2.2, §5.2): pick a `package.json` script or
/// type a command, name it, and add it to the Project. Nothing starts.
struct AddServiceView: View {
    let model: AppModel
    let projectID: UInt64
    let router: PopoverRouter
    let toaster: Toaster

    @State private var scripts: ProjectScripts?
    @State private var selection: ScriptKey?
    @State private var command = ""
    @State private var name = ""
    @State private var error: String?
    @State private var isAdding = false
    @FocusState private var focus: Field?

    private enum Field { case command, name }

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
                    Text("in \(project.name) · runs from \(Text(project.path.abbreviatingWithTilde).monospaced())")
                        .truncationMode(.middle)
                }
                PopoverScroll {
                    VStack(alignment: .leading, spacing: 10) {
                        if let scripts, scripts.hasPackageJson, !allScripts.isEmpty {
                            scriptsCard(scripts)
                        }
                        fieldsCard(project)
                    }
                }
                buttons(project)
            }
        }
        .task { await load() }
        .onChange(of: command) { _, newValue in
            // Editing the command by hand unselects the script.
            if let option = selectedOption, option.command != newValue {
                selection = nil
            }
            error = nil
        }
        .onChange(of: name) { error = nil }
    }

    // MARK: - Cards

    private func scriptsCard(_ scripts: ProjectScripts) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Scripts in package.json")
                .font(.caption)
                .foregroundStyle(.secondary)
            ForEach(scripts.groups, id: \.folder) { group in
                if let package = group.packageName {
                    Text(package)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                }
                FlowLayout(spacing: 6) {
                    ForEach(group.scripts, id: \.name) { option in
                        let key = ScriptKey(folder: group.folder, name: option.name)
                        ScriptChip(option: option, isSelected: selection == key) {
                            select(option, key: key)
                        }
                    }
                }
            }
        }
        .padding(EdgeInsets(top: 10, leading: 12, bottom: 10, trailing: 12))
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.background.secondary, in: RoundedRectangle(cornerRadius: 10))
        .padding(.horizontal, 2)
    }

    private func fieldsCard(_ project: ProjectState) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Card {
                CardRow(title: "Command") {
                    TextField("Command", text: $command, prompt: Text(verbatim: "bun run dev"))
                        .labelsHidden()
                        .font(.callout.monospaced())
                        .frame(width: 180)
                        .focused($focus, equals: .command)
                }
                CardRow(title: "Name") {
                    TextField("Name", text: $name, prompt: Text(verbatim: "dev"))
                        .labelsHidden()
                        .frame(width: 180)
                        .focused($focus, equals: .name)
                }
            }
            .controlSize(.small)
            if let message = nameProblem(in: project) ?? error {
                Text(message)
                    .font(.caption)
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
        scripts = loaded
        if let first = allScripts.first(where: { !$0.option.used }) {
            select(first.option, key: first.key)
        } else {
            focus = .command
        }
    }

    private func select(_ option: ScriptOption, key: ScriptKey) {
        selection = key
        command = option.command
        name = option.name
    }

    /// Why the name can't be used, shown under the fields.
    private func nameProblem(in project: ProjectState) -> String? {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        if trimmed.count > 40 {
            return String(localized: "Use at most 40 characters.")
        }
        let taken = project.services.contains { $0.name.lowercased() == trimmed.lowercased() }
        if !trimmed.isEmpty, taken {
            return String(localized: "Name already used in \(project.name)")
        }
        return nil
    }

    private func canAdd(to project: ProjectState) -> Bool {
        !isAdding
            && !command.trimmingCharacters(in: .whitespaces).isEmpty
            && !name.trimmingCharacters(in: .whitespaces).isEmpty
            && nameProblem(in: project) == nil
    }

    private func add(to project: ProjectState) {
        guard canAdd(to: project) else { return }
        isAdding = true
        let folder = selectedOption == nil ? nil : selection?.folder
        Task {
            let result = await model.addService(to: project.id, name: name, command: command, folder: folder)
            isAdding = false
            switch result {
            case .success(let service):
                toaster.show(String(localized: "Added \(project.name) › \(service.name)"))
                router.popToHome()
            case .failure(let failure):
                error = failure.userMessage
            }
        }
    }
}

/// A `package.json` script as a pill (design-handoff §2.2, ScriptChip).
private struct ScriptChip: View {
    let option: ScriptOption
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(option.name)
                .font(.caption.monospaced())
                .lineLimit(1)
                .truncationMode(.middle)
                .padding(.horizontal, 9)
                .frame(height: 24)
                .frame(maxWidth: 290)
                .foregroundStyle(isSelected ? Color.white : Color.primary)
                .background(
                    isSelected ? Color.accentColor : Color(nsColor: .quinarySystemFill),
                    in: Capsule()
                )
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .disabled(option.used)
        .opacity(option.used ? 0.4 : 1)
        .help(option.used ? String(localized: "Already a Service: \(option.command)") : option.command)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}
