import AppKit
import SwiftUI

/// The popover's root view (design-handoff §2.2): the current view of the
/// stack, then the fixed footer.
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
                .padding(.vertical, 4)
            quitConfirmation
            footer
        }
        .padding(6)
        .frame(width: 340)
        .background(Color(nsColor: .windowBackgroundColor).ignoresSafeArea())
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
            .background(.background.secondary, in: RoundedRectangle(cornerRadius: 9))
            .padding(EdgeInsets(top: 2, leading: 2, bottom: 4, trailing: 2))
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

    private var footer: some View {
        HStack(spacing: 6) {
            Text(verbatim: versionText)
                .font(.system(size: 11.5))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
            Group {
                Button("Settings") { openSettings(nil, false) }
                    .keyboardShortcut(",", modifiers: .command)
                Button("Quit", action: askToQuit)
                    .keyboardShortcut("q", modifiers: .command)
            }
            .buttonStyle(FooterButtonStyle())
            .fixedSize()
        }
        .padding(EdgeInsets(top: 3, leading: 10, bottom: 3, trailing: 4))
        .frame(minHeight: 28)
    }

    /// `0.1.0 · Bun 1.2.4 · Node 22.11.0`, leaving out what isn't known
    /// (design-handoff §6.5).
    private var versionText: String {
        func runtime(_ name: String, _ tool: Tool) -> String? {
            model.snapshot.environment.tools.first { $0.tool == tool }?.version.map { "\(name) \($0)" }
        }
        let app = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        return [app, runtime("Bun", .bun), runtime("Node", .node)]
            .compactMap(\.self)
            .joined(separator: " · ")
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

/// `Settings` and `Quit` (design-handoff §2.2): 22 pt tall with a hairline.
private struct FooterButtonStyle: ButtonStyle {
    private let shape = RoundedRectangle(cornerRadius: 6)

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 11.5))
            .padding(.horizontal, 8)
            .frame(height: 22)
            .background(configuration.isPressed ? Color.primary.opacity(0.08) : .clear, in: shape)
            .overlay(shape.strokeBorder(Color.primary.opacity(0.16)))
            .contentShape(shape)
    }
}
