import SwiftUI

/// Home (design-handoff §2.2): every Project, in registration order, with
/// its Services in creation order.
struct HomeView: View {
    let model: AppModel
    let router: PopoverRouter
    let openSettings: (SettingsPane?, Bool) -> Void

    /// The Service row with keyboard focus.
    @FocusState private var focusedService: UInt64?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
                .background {
                    // ⌘R restarts the focused row's Service (design-handoff §9).
                    Button("Restart") {
                        if let id = focusedService, let service = model.service(id), model.status(of: service) == .running {
                            model.restart(id)
                        }
                    }
                    .keyboardShortcut("r", modifiers: .command)
                    .opacity(0)
                    .frame(width: 0, height: 0)
                    .accessibilityHidden(true)
                }
            if model.snapshot.projects.isEmpty {
                emptyState
            } else {
                PopoverScroll {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(model.snapshot.projects, id: \.id) { project in
                            ProjectSection(model: model, project: project, router: router, focus: $focusedService)
                        }
                    }
                }
                .onKeyPress(.upArrow) { move(by: -1) }
                .onKeyPress(.downArrow) { move(by: 1) }
                .onKeyPress(.return) {
                    guard let id = focusedService else { return .ignored }
                    router.push(.service(id))
                    return .handled
                }
                .onKeyPress(.space) {
                    guard let id = focusedService, let service = model.service(id) else { return .ignored }
                    let status = model.status(of: service)
                    if status == .running || model.notResponding.contains(id) {
                        model.stop(id)
                    } else if !status.isBusy {
                        model.start(id)
                    }
                    return .handled
                }
            }
        }
        // Opening the popover puts the focus on the first Service row.
        .onAppear(perform: focusFirstRow)
        .onChange(of: router.openCount, focusFirstRow)
    }

    /// Service rows in the order Home shows them.
    private var rowOrder: [UInt64] {
        model.snapshot.projects.flatMap { $0.services.map(\.id) }
    }

    private func focusFirstRow() {
        focusedService = rowOrder.first
    }

    private func move(by step: Int) -> KeyPress.Result {
        focusedService = Self.focus(after: focusedService, in: rowOrder, step: step)
        return .handled
    }

    /// ↑ / ↓ between Service rows, stopping at the ends.
    static func focus(after current: UInt64?, in order: [UInt64], step: Int) -> UInt64? {
        guard !order.isEmpty else { return nil }
        guard let current, let index = order.firstIndex(of: current) else {
            return step > 0 ? order.first : order.last
        }
        return order[min(max(index + step, 0), order.count - 1)]
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text("Bunode")
                .font(.body.weight(.semibold))
                .frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityAddTraits(.isHeader)
            Text("\(model.statusSummary.running) running")
                .font(.callout)
                .foregroundStyle(.secondary)
            Button {
                router.push(.processes(focus: nil))
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "memorychip")
                        .font(.system(size: 14))
                    Text(verbatim: "\(model.snapshot.discovered.count)")
                        .font(.callout)
                }
                .padding(.horizontal, 6)
                .frame(height: 28)
                .contentShape(Rectangle())
            }
            .buttonStyle(.borderless)
            .foregroundStyle(.secondary)
            .help("Other Node and Bun processes, \(model.snapshot.discovered.count)")
            .accessibilityLabel("Other Node and Bun processes, \(model.snapshot.discovered.count)")
        }
        .frame(minHeight: 28)
        .padding(EdgeInsets(top: 4, leading: 10, bottom: 6, trailing: 2))
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Text("No Projects yet.")
                .foregroundStyle(.secondary)
            Button("Add Project…") {
                openSettings(.projects, true)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 30)
    }
}

private struct ProjectSection: View {
    let model: AppModel
    let project: ProjectState
    let router: PopoverRouter
    var focus: FocusState<UInt64?>.Binding

    /// Discovered Processes running in this Project's folder.
    private var externalProcesses: [DiscoveredProcess] {
        project.external.compactMap { pid in model.snapshot.discovered.first { $0.pid == pid } }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 4) {
                Text(project.name)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .accessibilityAddTraits(.isHeader)
                if project.isMissing {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .help("\(project.path.abbreviatingWithTilde) is missing.")
                        .accessibilityLabel("\(project.path.abbreviatingWithTilde) is missing.")
                }
                Spacer(minLength: 0)
                Button {
                    router.push(.addService(projectID: project.id))
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 12, weight: .medium))
                        .frame(width: 24, height: 24)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.borderless)
                .help("Add Service to \(project.name)")
                .accessibilityLabel("Add Service to \(project.name)")
            }
            .padding(EdgeInsets(top: 6, leading: 10, bottom: 0, trailing: 2))

            if project.services.isEmpty {
                Text("No Services. Click + to add one.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(EdgeInsets(top: 2, leading: 10, bottom: 6, trailing: 10))
            }
            ForEach(project.services, id: \.id) { service in
                ServiceRow(model: model, project: project, service: service) {
                    router.push(.service(service.id))
                }
                .focusable()
                .focused(focus, equals: service.id)
                if model.rowNotice?.serviceID == service.id {
                    RowNoticeView(model: model, router: router)
                }
            }
            ForEach(externalProcesses, id: \.pid) { process in
                ExternalRow(process: process) {
                    router.push(.processes(focus: process.pid))
                }
            }
        }
    }
}

struct ServiceRow: View {
    let model: AppModel
    let project: ProjectState
    let service: ServiceState
    /// Opens the Service view.
    let open: () -> Void

    private var status: RunStatus { model.status(of: service) }
    private var isNotResponding: Bool { model.notResponding.contains(service.id) }

    /// Crashed, Failed or Not responding.
    private var note: String? {
        isNotResponding ? String(localized: "Not responding") : status.rowNote
    }

    private var displayName: String {
        "\(project.name) › \(service.displayName)"
    }

    var body: some View {
        HStack(spacing: 0) {
            HStack(spacing: 8) {
                StatusDot(style: status.dotStyle)
                Text(service.displayName)
                    .lineLimit(1)
                    .truncationMode(.tail)
                if let note {
                    Text(note)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .help(service.lastExit ?? note)
                }
                Spacer(minLength: 4)
                PortLabel(service: service, status: status)
            }
            .padding(EdgeInsets(top: 0, leading: 10, bottom: 0, trailing: 4))
            .frame(maxWidth: .infinity, minHeight: 32)
            .contentShape(Rectangle())
            .onTapGesture(perform: open)

            RowAction(status: status, isNotResponding: isNotResponding, displayName: displayName) {
                if status == .running || isNotResponding {
                    model.stop(service.id)
                } else {
                    model.start(service.id)
                }
            }
            .padding(.trailing, 2)
        }
        .hoverHighlight()
        .help(displayName)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction(.default, open)
        .accessibilityAction(named: status == .running ? Text("Stop") : Text("Start")) {
            if status == .running {
                model.stop(service.id)
            } else if !status.isBusy {
                model.start(service.id)
            }
        }
        .contextMenu {
            if status == .running {
                Button("Restart") { model.restart(service.id) }
                Button("Stop") { model.stop(service.id) }
            } else if !status.isBusy {
                Button("Start") { model.start(service.id) }
            }
            Divider()
            Button("Open") { open() }
        }
    }

    private var accessibilityLabel: String {
        var label = "\(displayName), \(note ?? status.label)"
        if let port = service.shownPort(status: status)?.port {
            label += String(localized: ", port \(String(port))")
        }
        return label
    }
}

/// The button at the end of a Service row: Start, Stop, or a spinner while
/// the Supervisor works (design-handoff §2.2, §5.1).
private struct RowAction: View {
    let status: RunStatus
    /// A stuck Run gets its Stop button back, which force-kills.
    let isNotResponding: Bool
    let displayName: String
    let action: () -> Void

    private var showsStop: Bool { status == .running || isNotResponding }

    var body: some View {
        Group {
            if status.isBusy && !isNotResponding {
                ProgressView()
                    .controlSize(.small)
                    .scaleEffect(0.7)
                    .frame(width: 28, height: 26)
            } else {
                Button(action: action) {
                    Group {
                        if showsStop {
                            RoundedRectangle(cornerRadius: 2)
                                .fill(Color.primary)
                                .frame(width: 10, height: 10)
                        } else {
                            Image(systemName: "play.fill")
                                .font(.system(size: 10))
                                .foregroundStyle(Color.accentColor)
                        }
                    }
                    .frame(width: 28, height: 26)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.borderless)
                .help(showsStop ? "Stop \(displayName)" : "Start \(displayName)")
                .accessibilityLabel(showsStop ? "Stop \(displayName)" : "Start \(displayName)")
            }
        }
    }
}

/// The warning bar under a Service row (design-handoff §5.3, §7): the
/// Service's Port is taken, or its Project's folder is missing. One at a
/// time; its main button takes the focus.
struct RowNoticeView: View {
    let model: AppModel
    let router: PopoverRouter

    @FocusState private var mainButtonFocused: Bool

    var body: some View {
        Group {
            switch model.rowNotice {
            case .missingFolder(_, let projectID)?:
                if let project = model.project(projectID) {
                    InlineNotice(text: String(localized: "\(project.path.abbreviatingWithTilde) is missing.")) {
                        Button("Cancel") { model.dismissNotice() }
                            .buttonStyle(.borderless)
                        Button("Locate…") { locate(project) }
                            .buttonStyle(.borderedProminent)
                            .focused($mainButtonFocused)
                    }
                }
            case .portConflict(let serviceID, let port, let owner, let suggestion)?:
                InlineNotice(
                    text: conflictText(port: port, owner: owner),
                    detail: suggestion.map { String(localized: "Sets PORT=\(String($0)) for this Run. Tools that ignore PORT need their own flag.") }
                ) {
                    Button("Cancel") { model.dismissNotice() }
                        .buttonStyle(.borderless)
                    if let owner, owner.serviceId == nil {
                        Button("Show Process") {
                            router.push(.processes(focus: owner.pid))
                        }
                        .buttonStyle(.bordered)
                    }
                    if let suggestion {
                        Button("Use :\(String(suggestion))") { model.start(serviceID, port: suggestion) }
                            .buttonStyle(.borderedProminent)
                            .focused($mainButtonFocused)
                    }
                }
                // Once whatever held the port is gone, so is the bar.
                .task(id: port) {
                    while !Task.isCancelled {
                        try? await Task.sleep(for: .seconds(1))
                        if await !model.portInUse(port) {
                            model.dismissNotice()
                            return
                        }
                    }
                }
            case nil:
                EmptyView()
            }
        }
        .onAppear { mainButtonFocused = true }
    }

    /// `Port 4321 is used by “astro dev” from Terminal. Bunode won’t stop it.`
    private func conflictText(port: UInt16, owner: PortConflictOwner?) -> String {
        guard let owner else {
            return String(localized: "Port \(String(port)) is used by another program. Bunode won’t stop it.")
        }
        return String(localized: "Port \(String(port)) is used by “\(owner.command)” from \(owner.source). Bunode won’t stop it.")
    }

    private func locate(_ project: ProjectState) {
        Task {
            let parent = URL(filePath: project.path).deletingLastPathComponent()
            guard let url = await Panels.chooseFolder(prompt: String(localized: "Locate"), startingAt: parent, in: nil) else { return }
            // On failure (not a folder, already a Project) the bar stays.
            if model.relocateProject(project.id, to: url) != nil {
                NSSound.beep()
            }
        }
    }
}

/// A read-only row for a Discovered Process running in the Project's folder
/// (design-handoff §2.2, C5). No Start or Stop: Bunode didn't start it.
struct ExternalRow: View {
    let process: DiscoveredProcess
    let open: () -> Void

    var body: some View {
        Button(action: open) {
            HStack(spacing: 6) {
                Image(systemName: "memorychip")
                    .font(.system(size: 12))
                Text("External · \(process.command)")
                    .font(.callout)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 4)
                if let port = process.ports.first {
                    Text(verbatim: ":\(port)")
                        .font(.caption.monospaced())
                }
            }
            .foregroundStyle(.secondary)
            .padding(EdgeInsets(top: 0, leading: 10, bottom: 0, trailing: 8))
            .frame(height: 28)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .hoverHighlight()
        .help(process.command)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityHint("Shows it in Other processes")
    }

    private var accessibilityLabel: String {
        var label = String(localized: "External, \(process.command), from \(process.source)")
        if let port = process.ports.first {
            label += String(localized: ", port \(String(port))")
        }
        return label
    }
}
