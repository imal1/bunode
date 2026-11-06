import AppKit
import SwiftUI

/// The popover's root view (design-handoff §2.2): the current view of the
/// stack, then the fixed bottom menu.
struct PopoverView: View {
    let model: AppModel
    let router: PopoverRouter
    let toaster: Toaster
    let openSettings: (SettingsPane?, Bool) -> Void
    /// Quits; every Run is stopped by then.
    let quit: () -> Void
    /// Closes the popover.
    let close: () -> Void

    @State private var quitPhase = QuitPhase.idle

    enum QuitPhase { case idle, confirming, stopping }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ZStack(alignment: .top) {
                screen
                    .id(router.screen)
                    .transition(.push(from: router.isForward ? .trailing : .leading))
            }
            .clipped()
            Divider()
                .padding(.horizontal, 8)
                .padding(.top, 6)
                .padding(.bottom, 4)
            quitConfirmation
            MenuRow(title: "Settings…", key: ",", action: { openSettings(nil, false) })
            MenuRow(title: "Quit Bunode", key: "q", action: askToQuit)
        }
        .padding(6)
        .frame(width: 340)
        .toasts(toaster, maxWidth: 380)
        // Esc (design-handoff §9): an open confirmation's Cancel takes it
        // first (it is the cancel action); then the warning bar, then one
        // level back; on Home it closes the popover.
        .onExitCommand {
            if quitPhase == .confirming {
                quitPhase = .idle
            } else if model.rowNotice != nil {
                model.dismissNotice()
            } else if router.stack.count > 1 {
                router.back()
            } else {
                close()
            }
        }
    }

    /// Quit Bunode (design-handoff §5.6): straight away when nothing runs,
    /// otherwise after an inline confirmation, once every Run has stopped.
    private func askToQuit() {
        switch quitPhase {
        case .idle where model.liveCount == 0:
            quit()
        case .idle:
            withAnimation(.easeOut(duration: 0.15)) { quitPhase = .confirming }
        case .confirming:
            stopAndQuit()
        case .stopping:
            break
        }
    }

    private func stopAndQuit() {
        quitPhase = .stopping
        Task {
            await model.stopAllAndWait()
            quit()
        }
    }

    @ViewBuilder
    private var quitConfirmation: some View {
        switch quitPhase {
        case .idle:
            EmptyView()
        case .confirming:
            let count = model.liveCount
            VStack(alignment: .leading, spacing: 8) {
                Text("\(count) Services are running. Quitting stops every Run.")
                    .font(.callout)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 6) {
                    Spacer(minLength: 0)
                    Button("Cancel") { quitPhase = .idle }
                        .keyboardShortcut(.cancelAction)
                    Button("Stop \(count) Services and Quit", action: stopAndQuit)
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                }
                .controlSize(.small)
            }
            .padding(10)
            .background(.background.secondary, in: RoundedRectangle(cornerRadius: 10))
            .padding(EdgeInsets(top: 0, leading: 2, bottom: 6, trailing: 2))
            .onAppear {
                NSAccessibility.post(
                    element: NSApp as Any,
                    notification: .announcementRequested,
                    userInfo: [.announcement: String(localized: "\(count) Services are running. Quitting stops every Run."), .priority: NSAccessibilityPriorityLevel.high.rawValue]
                )
            }
        case .stopping:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Stopping all Runs…")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
            .padding(EdgeInsets(top: 4, leading: 10, bottom: 8, trailing: 10))
        }
    }

    @ViewBuilder
    private var screen: some View {
        switch router.screen {
        case .home:
            HomeView(model: model, router: router, openSettings: openSettings)
        case .addService(let projectID):
            AddServiceView(model: model, projectID: projectID, router: router, toaster: toaster)
        case .service(let serviceID):
            ServiceView(model: model, serviceID: serviceID, router: router, toaster: toaster)
        case .logs(let serviceID):
            LogsView(model: model, serviceID: serviceID, router: router, toaster: toaster)
        case .processes(let focus):
            OtherProcessesView(model: model, router: router, toaster: toaster, focus: focus)
        }
    }
}

/// A row of the popover's bottom menu: the title on the left, its ⌘
/// shortcut on the right.
private struct MenuRow: View {
    let title: LocalizedStringKey
    let key: Character
    let action: () -> Void

    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack {
                Text(title)
                Spacer()
                Text(verbatim: "⌘" + String(key).uppercased())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
            }
            .padding(.horizontal, 10)
            .frame(minHeight: 28)
            .contentShape(Rectangle())
            .background(
                isHovered ? Color(nsColor: .quaternarySystemFill) : .clear,
                in: RoundedRectangle(cornerRadius: 7)
            )
        }
        .buttonStyle(.plain)
        .keyboardShortcut(KeyEquivalent(key), modifiers: .command)
        .onHover { hovering in
            withAnimation(.easeOut(duration: 0.1)) {
                isHovered = hovering
            }
        }
    }
}
