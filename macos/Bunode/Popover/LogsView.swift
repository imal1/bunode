import AppKit
import SwiftUI

/// Logs (design-handoff §2.2): one Service's current or last Run, live.
struct LogsView: View {
    let model: AppModel
    let serviceID: UInt64
    let router: PopoverRouter
    let toaster: Toaster

    @State private var feed = LogFeed()
    @State private var query = ""
    @FocusState private var searchFocused: Bool

    private var isSearching: Bool { !query.trimmingCharacters(in: .whitespaces).isEmpty }

    private var service: ServiceState? { model.service(serviceID) }
    private var project: ProjectState? { service.flatMap { model.project($0.projectId) } }

    var body: some View {
        if let service, let project {
            let displayName = "\(project.name) › \(service.displayName)"
            VStack(alignment: .leading, spacing: 0) {
                TopBar(title: title(for: service), backLabel: String(localized: "Back to \(displayName)"), onBack: router.back) {
                    Text(displayName)
                } trailing: {
                    trailingButton(service)
                }
                searchField
                ZStack {
                    LogTableView(feed: feed, accessibilityName: String(localized: "Logs of \(displayName)"))
                    if let empty = emptyText(service) {
                        Text(empty)
                            .font(.callout)
                            .foregroundStyle(Color(nsColor: LogPalette.time))
                            .allowsHitTesting(false)
                    }
                }
                .frame(height: 400)
                .background(Color(nsColor: LogPalette.background), in: RoundedRectangle(cornerRadius: 9))
                .padding(.horizontal, 2)

                footer(service)
            }
            .onAppear {
                // Entering Logs always starts without a search.
                model.setLogFilter(serviceID, "")
                sync()
            }
            .onDisappear { model.setLogFilter(serviceID, "") }
            .onChange(of: query) { model.setLogFilter(serviceID, query) }
            .onChange(of: service.logFirst, sync)
            .onChange(of: service.logCount, sync)
            .onChange(of: service.logMatchFirst, sync)
            .onChange(of: service.logMatchCount, sync)
            .onChange(of: service.logFilterGeneration, sync)
            .background {
                // ⌘F and ⌘K (design-handoff §9), as invisible buttons.
                Group {
                    Button("Search") { searchFocused = true }
                        .keyboardShortcut("f", modifiers: .command)
                    Button("Clear") { model.clearLogs(serviceID) }
                        .keyboardShortcut("k", modifiers: .command)
                }
                .opacity(0)
                .frame(width: 0, height: 0)
                .accessibilityHidden(true)
            }
        }
    }

    private var searchField: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
            TextField("Search", text: $query)
                .textFieldStyle(.plain)
                .focused($searchFocused)
            if !query.isEmpty {
                Button {
                    query = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.borderless)
                .help("Clear search")
                .accessibilityLabel("Clear search")
            }
        }
        .padding(.horizontal, 8)
        .frame(height: 28)
        .background(Color(nsColor: .quinarySystemFill), in: RoundedRectangle(cornerRadius: 7))
        .padding(EdgeInsets(top: 0, leading: 2, bottom: 6, trailing: 2))
    }

    private func emptyText(_ service: ServiceState) -> String? {
        if isSearching {
            return service.logMatchCount == 0 ? String(localized: "No lines match.") : nil
        }
        return service.logCount == 0 ? String(localized: "No output yet.") : nil
    }

    private func title(for service: ServiceState) -> String {
        if let run = service.runNumber {
            return String(localized: "Logs · Run #\(String(run))")
        }
        return String(localized: "Logs · No Runs yet")
    }

    @ViewBuilder
    private func trailingButton(_ service: ServiceState) -> some View {
        let status = model.status(of: service)
        if status == .running {
            Button("Restart") { model.restart(service.id) }
                .keyboardShortcut("r", modifiers: .command)
                .controlSize(.small)
        } else if !status.isLive {
            Button("Start") { model.start(service.id) }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
        }
    }

    private func footer(_ service: ServiceState) -> some View {
        HStack(spacing: 4) {
            HStack(spacing: 0) {
                Text("\(Int(service.logCount)) lines")
                if isSearching {
                    Text(verbatim: " · ")
                    Text("\(Int(service.logMatchCount)) matches")
                }
                if service.logDropped > 0 {
                    Text(verbatim: " · ")
                    Text("\(Int(service.logDropped)) dropped")
                }
            }
            .font(.caption)
            .foregroundStyle(.secondary)
            Spacer()
            Button("Clear") { model.clearLogs(serviceID) }
            Button("Copy", action: copyAll)
                .disabled(service.logCount == 0)
            Button("Export…") { export(service) }
                .disabled(service.runNumber == nil)
        }
        .buttonStyle(.borderless)
        .controlSize(.small)
        .padding(EdgeInsets(top: 8, leading: 2, bottom: 2, trailing: 2))
    }

    private func sync() {
        guard let service = model.service(serviceID) else { return }
        if isSearching {
            feed.sync(first: service.logMatchFirst, count: service.logMatchCount, generation: service.logFilterGeneration) { from, max in
                model.logMatchingLines(serviceID, from: from, max: max)
            }
        } else {
            feed.sync(first: service.logFirst, count: service.logCount, generation: service.logFilterGeneration) { from, max in
                model.logLines(serviceID, from: from, max: max)
            }
        }
    }

    /// Saves the Run's full log, not just the lines kept in memory.
    private func export(_ service: ServiceState) {
        guard let project else { return }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = AppModel.exportFileName(project: project.name, service: service.name, run: service.runNumber)
        panel.canCreateDirectories = true
        panel.isExtensionHidden = false
        NSApp.activate()
        Task {
            guard await panel.begin() == .OK, let url = panel.url else { return }
            if let problem = await model.exportLog(serviceID, to: url) {
                toaster.show(problem)
            } else {
                toaster.show(String(localized: "Saved \(url.lastPathComponent)"))
            }
        }
    }

    private func copyAll() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(LogPalette.plainText(of: feed.mirror.lines), forType: .string)
    }
}
