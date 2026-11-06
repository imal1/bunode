import AppKit
import SwiftUI

/// Other processes (design-handoff §2.2, §5.2): Node and Bun processes
/// started outside Bunode. Watch only: Stop… sends SIGTERM after a
/// confirmation, and a process that ignores it can be force killed after
/// a second one. Never in bulk, never restarted.
struct OtherProcessesView: View {
    let model: AppModel
    let router: PopoverRouter
    let toaster: Toaster
    /// Scroll to this process when the view opens (from an External row).
    let focus: UInt32?

    /// The process whose Stop… confirmation is open.
    @State private var confirming: DiscoveredProcess?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            TopBar(title: String(localized: "Other processes"), onBack: router.back) {
                Text("Started outside Bunode · watch only")
            }
            if model.snapshot.discovered.isEmpty {
                Text("No other Node or Bun processes.")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 30)
            } else {
                ScrollViewReader { proxy in
                    PopoverScroll {
                        Card {
                            ForEach(model.snapshot.discovered, id: \.pid) { process in
                                ProcessRow(
                                    process: process,
                                    isConfirming: confirming?.pid == process.pid,
                                    confirm: { confirming = process },
                                    cancel: { confirming = nil },
                                    sendTerm: { sendTerm(process) }
                                )
                                .id(process.pid)
                            }
                        }
                    }
                    .onAppear {
                        if let focus {
                            proxy.scrollTo(focus, anchor: .center)
                        }
                    }
                }
            }
        }
        .onAppear { model.rescan() }
    }

    private func sendTerm(_ process: DiscoveredProcess) {
        confirming = nil
        if let problem = model.stopDiscovered(process) {
            toaster.show(problem)
            return
        }
        toaster.show(String(localized: "SIGTERM sent to PID \(String(process.pid))"))
        // Still there 5 s later: offer a force kill (design-handoff §2.4).
        Task {
            try? await Task.sleep(for: .seconds(5))
            model.rescan()
            try? await Task.sleep(for: .milliseconds(500))
            model.refresh()
            guard model.isStillRunning(process) else { return }
            confirmForceKill(process)
        }
    }

    private func confirmForceKill(_ process: DiscoveredProcess) {
        let name = process.command.split(separator: " ").first.map(String.init) ?? "node"
        let alert = NSAlert()
        alert.messageText = String(localized: "Force kill \(name) (PID \(String(process.pid)))?")
        alert.informativeText = String(localized: "Bunode didn’t start this process. It did not exit 5 s after SIGTERM. Force killing it may lose unsaved work.")
        let details = [process.command, process.cwd?.abbreviatingWithTilde, String(localized: "started from \(process.source)")]
            .compactMap { $0 }
            .joined(separator: " · ")
        let field = NSTextField(wrappingLabelWithString: details)
        field.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        field.isSelectable = true
        field.preferredMaxLayoutWidth = 260
        field.frame.size.width = 260
        alert.accessoryView = field
        // Keep Running is the default: force killing is the destructive choice.
        alert.addButton(withTitle: String(localized: "Keep Running"))
        let kill = alert.addButton(withTitle: String(localized: "Force Kill"))
        kill.hasDestructiveAction = true
        NSApp.activate()
        if alert.runModal() == .alertSecondButtonReturn {
            model.forceKillDiscovered(process)
        }
    }
}

private struct ProcessRow: View {
    let process: DiscoveredProcess
    let isConfirming: Bool
    let confirm: () -> Void
    let cancel: () -> Void
    let sendTerm: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(process.command)
                        .font(.body.monospaced())
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .help(process.command)
                    Text(subtitle)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
                if process.exiting {
                    Text("Exiting…")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else if !isConfirming {
                    Button("Stop…", action: confirm)
                        .controlSize(.small)
                }
            }
            .padding(.vertical, 4)
            .padding(.horizontal, 12)
            .frame(minHeight: 44)
            .opacity(process.exiting ? 0.45 : 1)
            .accessibilityElement(children: .combine)

            if isConfirming {
                InlineNotice(text: String(localized: "Send SIGTERM to PID \(String(process.pid))? Bunode didn’t start it and can’t bring it back.")) {
                    Button("Cancel", action: cancel)
                        .buttonStyle(.borderless)
                    Button("Send SIGTERM", action: sendTerm)
                        .buttonStyle(.borderedProminent)
                        .tint(.primary)
                }
                .padding(.horizontal, 8)
            }
        }
    }

    /// `Terminal · PID 48121 · :4321`
    private var subtitle: String {
        var parts = [process.source, String(localized: "PID \(String(process.pid))")]
        parts += process.ports.map { ":\($0)" }
        return parts.joined(separator: " · ")
    }
}
