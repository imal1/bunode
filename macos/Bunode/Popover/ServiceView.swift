import AppKit
import SwiftUI

/// The Service view (design-handoff §2.2, §5.4, §5.5): status and
/// Start/Stop at the top, then the Service's settings, saved as soon as a
/// field loses focus or Return is pressed, its Logs, and Delete.
struct ServiceView: View {
    let model: AppModel
    let serviceID: UInt64
    let router: PopoverRouter
    let toaster: Toaster

    @State private var command = ""
    @State private var port = ""
    @State private var problem: String?
    @State private var confirmingDelete = false
    @State private var isDeleting = false
    @FocusState private var focus: Field?

    private enum Field { case command, port }

    private var service: ServiceState? { model.service(serviceID) }
    private var project: ProjectState? { service.flatMap { model.project($0.projectId) } }

    var body: some View {
        if let service, let project {
            let status = model.status(of: service)
            let displayName = "\(project.name) › \(service.name)"
            VStack(alignment: .leading, spacing: 0) {
                TopBar(title: displayName, onBack: router.back) {
                    StatusLine(service: service, status: status, isNotResponding: model.notResponding.contains(serviceID))
                } trailing: {
                    StartStopButton(model: model, service: service, status: status)
                }
                PopoverScroll {
                    VStack(alignment: .leading, spacing: 10) {
                        Card {
                            CardRow(title: "Command") {
                                TextField("Command", text: $command, prompt: Text(verbatim: "bun run dev"))
                                    .labelsHidden()
                                    .font(.callout.monospaced())
                                    .frame(width: 180)
                                    .focused($focus, equals: .command)
                                    .onSubmit { save(service) }
                            }
                            CardRow(title: "Folder") {
                                HStack(spacing: 6) {
                                    Text(folderLabel(service, project))
                                        .font(.caption.monospaced())
                                        .foregroundStyle(.secondary)
                                        .lineLimit(1)
                                        .truncationMode(.middle)
                                        .help(folderPath(service, project))
                                    Button("Change…") { chooseFolder(service, project) }
                                }
                            }
                            CardRow(title: "Port", help: "Checked before each start") {
                                TextField("Port", text: $port, prompt: Text("none"))
                                    .labelsHidden()
                                    .multilineTextAlignment(.trailing)
                                    .font(.callout.monospaced())
                                    .frame(width: 64)
                                    .focused($focus, equals: .port)
                                    .onSubmit { save(service) }
                                    .onChange(of: port) { _, value in
                                        let digits = String(value.filter(\.isNumber).prefix(5))
                                        if digits != value { port = digits }
                                    }
                            }
                        }
                        Card {
                            CardRow(title: "Run through shell", help: "For pipes, && and globs") {
                                Toggle("Run through shell", isOn: Binding(
                                    get: { service.runThroughShell },
                                    set: { save(service, shell: $0) }
                                ))
                                .labelsHidden()
                                .toggleStyle(.switch)
                            }
                        }
                        if let problem {
                            Text(problem)
                                .font(.caption)
                                .foregroundStyle(Color(nsColor: .systemRed))
                                .padding(.horizontal, 14)
                        }
                        Card {
                            Button {
                                router.push(.logs(serviceID))
                            } label: {
                                CardRow(title: "Logs") {
                                    HStack(spacing: 6) {
                                        Text("\(Int(service.logCount)) lines")
                                            .foregroundStyle(.secondary)
                                        Image(systemName: "chevron.right")
                                            .font(.system(size: 9, weight: .semibold))
                                            .foregroundStyle(.secondary)
                                    }
                                }
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                        }
                        if service.hasPendingConfig {
                            PendingConfigBar { model.restart(serviceID) }
                        }
                        deleteSection(service, project, status: status)
                    }
                    .controlSize(.small)
                }
            }
            .onAppear { load(service) }
            .onDisappear { save(service) }
            .onChange(of: focus) { old, _ in
                if old != nil { save(service) }
            }
        }
    }

    // MARK: - Folder

    private func folderPath(_ service: ServiceState, _ project: ProjectState) -> String {
        guard let folder = service.folder else { return project.path }
        return URL(filePath: project.path).appending(path: folder).path(percentEncoded: false)
    }

    private func folderLabel(_ service: ServiceState, _ project: ProjectState) -> String {
        folderPath(service, project).abbreviatingWithTilde
    }

    /// Only the Project's folder or one inside it (requirements §3.1).
    private func chooseFolder(_ service: ServiceState, _ project: ProjectState) {
        Task {
            let start = URL(filePath: folderPath(service, project), directoryHint: .isDirectory)
            guard let url = await Panels.chooseFolder(prompt: String(localized: "Choose"), startingAt: start, in: nil) else { return }
            let root = URL(filePath: project.path, directoryHint: .isDirectory).standardizedFileURL.path(percentEncoded: false)
            let chosen = url.standardizedFileURL.path(percentEncoded: false)
            let rootPrefix = root.hasSuffix("/") ? root : root + "/"
            let chosenSlash = chosen.hasSuffix("/") ? chosen : chosen + "/"
            guard chosenSlash.hasPrefix(rootPrefix) else {
                toaster.show(String(localized: "Choose the Project's folder or a folder inside it."))
                return
            }
            let relative = String(chosenSlash.dropFirst(rootPrefix.count)).trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            save(service, folder: .some(relative.isEmpty ? nil : relative))
        }
    }

    // MARK: - Saving

    private func load(_ service: ServiceState) {
        command = service.command
        port = service.port.map { String($0) } ?? ""
    }

    /// Saves the fields; `shell` and `folder` override the current values.
    private func save(_ service: ServiceState, shell: Bool? = nil, folder: String?? = nil) {
        guard let current = model.service(service.id) else { return }
        let trimmedPort = port.trimmingCharacters(in: .whitespaces)
        var parsedPort: UInt16?
        if !trimmedPort.isEmpty {
            guard let value = Int(trimmedPort), (1...65535).contains(value) else {
                problem = String(localized: "Use a port from 1 to 65535.")
                return
            }
            parsedPort = UInt16(value)
        }
        let edit = ServiceEdit(
            command: command,
            folder: folder ?? current.folder,
            port: parsedPort,
            runThroughShell: shell ?? current.runThroughShell
        )
        problem = model.updateService(service.id, edit)
    }

    // MARK: - Delete

    @ViewBuilder
    private func deleteSection(_ service: ServiceState, _ project: ProjectState, status: RunStatus) -> some View {
        if confirmingDelete {
            let files = project.path.abbreviatingWithTilde
            let message = status.isLive
                ? String(localized: "It is running and will be stopped first. Only the Service is removed; files in \(files) stay.")
                : String(localized: "Only the Service is removed; files in \(files) stay.")
            VStack(alignment: .leading, spacing: 8) {
                Text(message)
                    .font(.callout)
                HStack(spacing: 6) {
                    Spacer()
                    if isDeleting {
                        ProgressView().controlSize(.small)
                    }
                    Button("Cancel") { confirmingDelete = false }
                        .keyboardShortcut(.cancelAction)
                        .disabled(isDeleting)
                    Button(status.isLive ? "Stop and Delete" : "Delete", role: .destructive) {
                        delete(service)
                    }
                    .disabled(isDeleting)
                }
            }
            .padding(10)
            .background(.background.secondary, in: RoundedRectangle(cornerRadius: 10))
            .padding(EdgeInsets(top: 0, leading: 2, bottom: 6, trailing: 2))
        } else {
            Button("Delete Service…") { confirmingDelete = true }
                .buttonStyle(.borderless)
                .font(.callout)
                .foregroundStyle(Color(nsColor: .systemRed))
                .padding(EdgeInsets(top: 2, leading: 10, bottom: 6, trailing: 10))
        }
    }

    private func delete(_ service: ServiceState) {
        isDeleting = true
        Task {
            let message = await model.deleteService(service.id)
            isDeleting = false
            confirmingDelete = false
            toaster.show(message)
            router.popToHome()
        }
    }
}

/// "Saved. Applies on the next Run." + Restart now (design-handoff §5.4).
private struct PendingConfigBar: View {
    let restart: () -> Void

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "info.circle")
            Text("Saved. Applies on the next Run.")
            Spacer(minLength: 4)
            Button("Restart now", action: restart)
                .buttonStyle(.borderless)
                .fontWeight(.semibold)
        }
        .font(.callout)
        .foregroundStyle(Color.accentColor)
        .padding(.vertical, 7)
        .padding(.horizontal, 10)
        .background(Color.accentColor.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
        .padding(.horizontal, 2)
        .accessibilityElement(children: .combine)
    }
}

/// `Running · Run #14` with a small status dot.
struct StatusLine: View {
    let service: ServiceState
    let status: RunStatus
    let isNotResponding: Bool

    var body: some View {
        HStack(spacing: 4) {
            StatusDot(style: status.dotStyle, size: 6)
            Text(text)
        }
    }

    private var text: String {
        var parts = [isNotResponding ? String(localized: "Not responding") : status.label]
        if status.isLive, let run = service.runNumber {
            parts.append(String(localized: "Run #\(String(run))"))
        }
        return parts.joined(separator: " · ")
    }
}

/// Start (primary) or Stop (secondary) for a sub-view's top bar; nothing
/// while the Supervisor is busy.
struct StartStopButton: View {
    let model: AppModel
    let service: ServiceState
    let status: RunStatus

    private var isNotResponding: Bool { model.notResponding.contains(service.id) }

    var body: some View {
        Group {
            if status == .running || isNotResponding {
                Button("Stop") { model.stop(service.id) }
            } else if !status.isBusy {
                Button("Start") { model.start(service.id) }
                    .buttonStyle(.borderedProminent)
            }
        }
        .controlSize(.small)
    }
}
