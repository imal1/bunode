import SwiftUI

/// Home (design-handoff §2.2): every Project, in registration order, with
/// its Services in creation order.
struct HomeView: View {
    let model: AppModel
    let router: PopoverRouter
    let openSettings: (SettingsPane?, Bool) -> Void

    /// The Service row with keyboard focus.
    @FocusState private var focusedService: UInt64?
    /// The list itself: it takes the keyboard when the popover opens, so no
    /// row is selected until an arrow key picks one (design-handoff §9).
    @FocusState private var listFocused: Bool

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
            Divider()
                .padding(.horizontal, 8)
                .padding(.bottom, 4)
            if model.snapshot.projects.isEmpty {
                emptyState
            } else if rowOrder.isEmpty {
                Text("No commands yet.")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 30)
                    .padding(.horizontal, 16)
            } else {
                PopoverScroll {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(model.snapshot.projects, id: \.id) { project in
                            ForEach(project.services, id: \.id) { service in
                                ServiceRow(model: model, project: project, service: service, isFocused: focusedService == service.id) {
                                    router.push(.service(service.id))
                                }
                                .focusable()
                                .focusEffectDisabled()
                                .focused($focusedService, equals: service.id)
                                if model.rowNotice?.serviceID == service.id {
                                    // Lined up with the row's text column.
                                    RowNoticeView(model: model, router: router)
                                        .padding(EdgeInsets(top: 2, leading: 50, bottom: 6, trailing: 8))
                                }
                            }
                        }
                    }
                }
                .background {
                    // Beside the rows, not around them: a disabled focus
                    // effect reaches everything inside the view it is on.
                    Color.clear
                        .focusable()
                        .focusEffectDisabled()
                        .focused($listFocused)
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
            Divider()
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
            OtherProcessesRow(count: model.snapshot.discovered.count) {
                router.push(.processes(focus: nil))
            }
        }
        .onAppear(perform: focusList)
        .onChange(of: router.openCount, focusList)
    }

    /// Service rows in the order Home shows them.
    private var rowOrder: [UInt64] {
        model.snapshot.projects.flatMap { $0.services.map(\.id) }
    }

    private func focusList() {
        focusedService = nil
        listFocused = true
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
        let summary = model.statusSummary
        return HStack(alignment: .top, spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text("Bunode")
                        .font(.system(size: 14, weight: .semibold))
                        .accessibilityAddTraits(.isHeader)
                    Circle()
                        .fill(Color(nsColor: summary.aggregate.color))
                        .frame(width: 7, height: 7)
                        .accessibilityHidden(true)
                }
                Text(verbatim: "\(String(localized: "\(summary.running) running")) · \(String(localized: "\(model.services.count) Services"))")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            if let first = model.snapshot.projects.first {
                AddServiceLink {
                    router.push(.addService(projectID: first.id))
                }
            }
        }
        .padding(EdgeInsets(top: 4, leading: 10, bottom: 8, trailing: 8))
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

/// `Add Service` in the Home header: a text link, underlined on hover.
private struct AddServiceLink: View {
    let action: () -> Void

    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            Text("Add Service")
                .font(.system(size: 12.5))
                .underline(isHovered)
                .foregroundStyle(Color.accentColor)
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

/// The fixed row under the Service list (design-handoff §2.2): shown even
/// when nothing runs outside Bunode.
private struct OtherProcessesRow: View {
    let count: Int
    let open: () -> Void

    var body: some View {
        Button(action: open) {
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(.secondary)
                    .frame(width: 32, height: 32)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 9))
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Other processes")
                        .font(.system(size: 13, weight: .medium))
                    Text(count == 0 ? "None right now" : "\(count) started outside Bunode")
                        .font(.system(size: 11.5))
                        .foregroundStyle(.secondary)
                }
                .lineLimit(1)
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(.tertiary)
                    .frame(width: 28, height: 28)
                    .accessibilityHidden(true)
            }
            .padding(EdgeInsets(top: 0, leading: 8, bottom: 0, trailing: 6))
            .frame(height: 46)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .hoverHighlight(cornerRadius: 9)
    }
}

/// A Service on Home (design-handoff §2.2, §5.1): its status tile, then its
/// command over where it lives and what its Run is doing.
struct ServiceRow: View {
    let model: AppModel
    let project: ProjectState
    let service: ServiceState
    /// The list's keyboard selection is on this row.
    let isFocused: Bool
    /// Opens the Service view.
    let open: () -> Void

    @State private var isHovered = false

    private var status: RunStatus { model.status(of: service) }
    private var isNotResponding: Bool { model.notResponding.contains(service.id) }
    /// A live Run keeps its status even if its folder went away.
    private var isFolderMissing: Bool { project.isMissing && !status.isLive }

    private var displayName: String {
        "\(project.name) › \(service.displayName)"
    }

    private var statusText: String {
        if isFolderMissing { return String(localized: "Folder missing") }
        if isNotResponding { return String(localized: "Not responding") }
        return status.label
    }

    private var isStatusError: Bool {
        isFolderMissing || isNotResponding || status.dotStyle == .failed
    }

    /// A stuck Run offers Stop too, which force-kills.
    private var offersStop: Bool { status == .running || isNotResponding }

    /// A busy Run has nothing to press unless it is stuck.
    private var hasAction: Bool { offersStop || !status.isBusy }

    private func startOrStop() {
        if offersStop {
            model.stop(service.id)
        } else if !status.isBusy {
            model.start(service.id)
        }
    }

    var body: some View {
        HStack(spacing: 0) {
            HStack(spacing: 10) {
                StatusTile(style: status.dotStyle)
                VStack(alignment: .leading, spacing: 1) {
                    HStack(spacing: 6) {
                        Text(verbatim: ServiceState.shortCommand(service.command))
                            .font(.system(size: 13, weight: .medium))
                            .lineLimit(1)
                            .truncationMode(.tail)
                        PortPill(ports: service.shownPorts(status: status))
                    }
                    secondLine
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(EdgeInsets(top: 0, leading: 8, bottom: 0, trailing: 6))
            .frame(height: 46)
            .contentShape(Rectangle())
            .onTapGesture(perform: open)

            RowAction(
                hasButton: hasAction,
                offersStop: offersStop,
                isRevealed: isHovered || isFocused,
                displayName: displayName,
                action: startOrStop
            )
            .padding(.trailing, 6)
        }
        .hoverHighlight(cornerRadius: 9, isActive: isFocused)
        .onHover { isHovered = $0 }
        .help(tooltip)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction(.default, open)
        .accessibilityActions {
            if hasAction {
                Button(action: startOrStop) { offersStop ? Text("Stop") : Text("Start") }
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

    /// `{project}`, `{project} › {folder}`, then ` · {status}` unless
    /// Stopped. When short of room the Project name gives way first.
    private var secondLine: some View {
        HStack(spacing: 0) {
            Text(verbatim: service.folder.map { "\(project.name) › \($0)" } ?? project.name)
                .truncationMode(.tail)
            if status != .stopped || isFolderMissing {
                HStack(spacing: 0) {
                    Text(verbatim: " · ")
                    Text(verbatim: statusText)
                        .foregroundStyle(isStatusError ? Color.statusErrorText : .secondary)
                }
                .fixedSize()
            }
        }
        .font(.system(size: 11.5))
        .foregroundStyle(.secondary)
        .lineLimit(1)
    }

    /// The full command and folder; a Crashed or Failed Run adds how it ended.
    private var tooltip: String {
        var lines = [service.command, project.runPath(folder: service.folder).abbreviatingWithTilde]
        if status.dotStyle == .failed, let lastExit = service.lastExit {
            lines.append(lastExit)
        }
        return lines.joined(separator: "\n")
    }

    private var accessibilityLabel: String {
        var label = "\(displayName), \(statusText)"
        if let port = service.shownPorts(status: status).first {
            label += String(localized: ", port \(String(port))")
        }
        return label
    }
}

/// `:3000`, `+N` when the Run listens on more; it never compresses, so the
/// command truncates first.
private struct PortPill: View {
    let ports: [UInt16]

    var body: some View {
        if let first = ports.first {
            Text(verbatim: ports.count > 1 ? ":\(first) +\(ports.count - 1)" : ":\(first)")
                .font(.system(size: 10.5).monospacedDigit())
                .foregroundStyle(.secondary)
                .padding(.horizontal, 5)
                .frame(height: 16)
                .background(.quinary, in: RoundedRectangle(cornerRadius: 4))
                .fixedSize()
                .help(ports.map { ":\($0)" }.joined(separator: " "))
        }
    }
}

/// The end of a Service row (design-handoff §2.2, §5.1): an arrow at rest,
/// Start or Stop on hover or keyboard focus. Stacked, so the row keeps its
/// width. A busy Run has no button and keeps the arrow.
private struct RowAction: View {
    let hasButton: Bool
    let offersStop: Bool
    let isRevealed: Bool
    let displayName: String
    let action: () -> Void

    /// Tab can land on the button itself.
    @FocusState private var isFocused: Bool

    private var showsButton: Bool { hasButton && (isRevealed || isFocused) }

    var body: some View {
        ZStack {
            Image(systemName: "chevron.right")
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(.tertiary)
                .opacity(showsButton ? 0 : 1)
            if hasButton {
                Button(action: action) {
                    Group {
                        if offersStop {
                            RoundedRectangle(cornerRadius: 2)
                                .fill(Color.primary)
                                .frame(width: 10, height: 10)
                        } else {
                            Image(systemName: "play.fill")
                                .font(.system(size: 10))
                                .foregroundStyle(Color.accentColor)
                        }
                    }
                    .frame(width: 28, height: 28)
                    .contentShape(Rectangle())
                    .hoverHighlight(cornerRadius: 7, isActive: isFocused)
                }
                .buttonStyle(.borderless)
                .focused($isFocused)
                .opacity(showsButton ? 1 : 0)
                .help(offersStop ? "Stop \(displayName)" : "Start \(displayName)")
                .accessibilityLabel(offersStop ? "Stop \(displayName)" : "Start \(displayName)")
            }
        }
        .frame(width: 28, height: 28)
        .animation(.easeInOut(duration: 0.1), value: showsButton)
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
                    InlineNotice(text: String(localized: "\(project.path.abbreviatingWithTilde) is missing."), verticalPadding: 8) {
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
                    detail: suggestion.map { String(localized: "Sets PORT=\(String($0)) for this Run. Tools that ignore PORT need their own flag.") },
                    verticalPadding: 8
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

extension ServiceState {
    /// What a row and the Service view title call a command (design-handoff
    /// §2.2): the script name of a preset, otherwise the whole command.
    static func shortCommand(_ command: String) -> String {
        let preset = /(?:bun|npm|pnpm|yarn)(?:\s+run)?\s+((?:dev|start|serve|preview|build|test|lint)(?:[:-]\S+)?)/
        return command.wholeMatch(of: preset).map { String($0.1) } ?? command
    }

    /// The ports shown for a Service (design-handoff §2.2, §5.1, §11): only
    /// what a running Run listens on, on the row and in the Service view's
    /// status line. The configured Port is a field of the Service view.
    func shownPorts(status: RunStatus) -> [UInt16] {
        status == .running ? livePorts : []
    }
}

