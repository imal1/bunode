import AppKit
import SwiftUI

enum SettingsPane: String, CaseIterable, Identifiable {
    case general, projects, advanced

    var id: Self { self }

    var title: LocalizedStringKey {
        switch self {
        case .general: "General"
        case .projects: "Projects"
        case .advanced: "Advanced"
        }
    }

    var symbol: String {
        switch self {
        case .general: "gearshape"
        case .projects: "folder"
        case .advanced: "slider.horizontal.3"
        }
    }
}

/// Which pane the Settings window shows. Owned by the window controller so
/// the app can open a specific pane.
@MainActor
@Observable
final class SettingsNavigation {
    var pane: SettingsPane? = .general
    /// Set to ask the Projects pane to open the folder picker once.
    var pendingAddProject = false
}

/// The Settings window's content (design-handoff §2.3): a sidebar with
/// General, Projects and Advanced, and the selected pane.
struct SettingsView: View {
    let model: AppModel
    @Bindable var navigation: SettingsNavigation
    @State private var toaster = Toaster()

    var body: some View {
        HStack(spacing: 0) {
            SettingsSidebar(selection: $navigation.pane)
                .frame(width: 168)
            Divider()
            detail
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .background(Color(nsColor: .windowBackgroundColor))
        }
        .ignoresSafeArea()
        .frame(
            width: SettingsWindowController.contentSize.width,
            height: SettingsWindowController.contentSize.height
        )
        .toasts(toaster, maxWidth: 460)
    }

    private var detail: some View {
        let pane = navigation.pane ?? .general
        return VStack(alignment: .leading, spacing: 0) {
            Text(pane.title)
                .font(.title3.bold())
                .padding(EdgeInsets(top: 20, leading: 22, bottom: 6, trailing: 22))
                .accessibilityAddTraits(.isHeader)
            switch pane {
            case .general:
                GeneralPane(model: model)
            case .projects:
                ProjectsPane(model: model, navigation: navigation, toaster: toaster)
            case .advanced:
                AdvancedPane(model: model, toaster: toaster)
            }
        }
    }
}

private struct SettingsSidebar: View {
    @Binding var selection: SettingsPane?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            List(SettingsPane.allCases, selection: $selection) { pane in
                Label(pane.title, systemImage: pane.symbol)
                    .font(.body.weight(.medium))
                    .frame(minHeight: 24)
            }
            .listStyle(.sidebar)
            .scrollContentBackground(.hidden)
            .scrollDisabled(true)

            Link(destination: URL(string: "https://github.com/imal1/bunode")!) {
                HStack(spacing: 3) {
                    Text("Source code")
                    Image(systemName: "arrow.up.right")
                        .font(.caption)
                }
                .font(.callout)
            }
            .foregroundStyle(.secondary)
            .padding(EdgeInsets(top: 0, leading: 18, bottom: 14, trailing: 8))
        }
        // Room for the traffic lights above the list.
        .padding(.top, 38)
        .background(VisualEffectBackground(material: .sidebar))
    }
}

struct GeneralPane: View {
    let model: AppModel

    var body: some View {
        Form {
            Section("App") {
                Toggle(isOn: Binding(
                    get: { model.snapshot.settings.showRunningCount },
                    set: { model.setShowRunningCount($0) }
                )) {
                    Text("Show running count")
                    Text("Next to the menu bar icon.")
                }
            }
        }
        .formStyle(.grouped)
        .toggleStyle(.switch)
        .controlSize(.small)
    }
}

struct AdvancedPane: View {
    let model: AppModel
    let toaster: Toaster

    var body: some View {
        Form {
            Section("Env") {
                ShellEnvironmentRow(model: model, toaster: toaster)
                ForEach(model.snapshot.environment.tools, id: \.tool) { tool in
                    ToolRow(model: model, tool: tool, toaster: toaster)
                }
            }
            Section("Stop") {
                Picker(selection: Binding(
                    get: { model.snapshot.settings.forceStopAfterSecs },
                    set: { model.setForceStopAfter($0) }
                )) {
                    ForEach([UInt32(3), 5, 10, 30], id: \.self) { seconds in
                        Text("\(seconds) s").tag(seconds)
                    }
                } label: {
                    Text("Force stop after")
                    Text("Stop asks nicely first, then force-quits.")
                }
                .pickerStyle(.menu)
                Picker(selection: Binding(
                    get: { model.snapshot.settings.logLinesKept },
                    set: { model.setLogLinesKept($0) }
                )) {
                    ForEach([UInt32(5_000), 10_000, 50_000], id: \.self) { lines in
                        Text(lines.formatted()).tag(lines)
                    }
                } label: {
                    Text("Log lines kept")
                    Text("Per Service. Export saves the full log.")
                }
                .pickerStyle(.menu)
                LabeledContent {
                    Button("Open") {
                        let folder = AppModel.defaultDiagnosticsDirectory
                        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
                        NSWorkspace.shared.open(folder)
                    }
                } label: {
                    Text("Diagnostics")
                    Text("Supervisor and runner logs.")
                }
            }
        }
        .formStyle(.grouped)
        .controlSize(.small)
    }
}

/// Advanced › Shell environment: where Runs get PATH and friends from.
private struct ShellEnvironmentRow: View {
    let model: AppModel
    let toaster: Toaster
    @State private var reloadRequested = false

    private var environment: EnvironmentState { model.snapshot.environment }

    var body: some View {
        LabeledContent {
            Button {
                reloadRequested = true
                model.reloadEnvironment()
            } label: {
                if environment.isLoading {
                    HStack(spacing: 4) {
                        ProgressView().controlSize(.mini)
                        Text("Reloading")
                    }
                } else {
                    Text("Reload")
                }
            }
            .disabled(environment.isLoading)
        } label: {
            Text("Shell environment")
            if let problem = environment.problem {
                Text(problem)
                    .foregroundStyle(Color(nsColor: .systemRed))
                    .help(problem)
            } else {
                Text(summary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
        }
        .onChange(of: environment.isLoading) { wasLoading, isLoading in
            if wasLoading, !isLoading, reloadRequested {
                reloadRequested = false
                if environment.problem == nil {
                    toaster.show(String(localized: "Environment reloaded. New Runs get it."))
                }
            }
        }
    }

    /// "87 variables from /bin/zsh · today 09:58"
    private var summary: String {
        guard let loadedAt = environment.loadedAt else {
            return String(localized: "Reading from \(environment.shell)…")
        }
        let date = Date(timeIntervalSince1970: TimeInterval(loadedAt))
        return String(localized: "\(Int(environment.variableCount)) variables from \(environment.shell) · \(EnvironmentTime.describe(date))")
    }
}

enum EnvironmentTime {
    /// "just now", "today 09:58", or a date and time.
    static func describe(_ date: Date, now: Date = .now) -> String {
        if now.timeIntervalSince(date) < 60 {
            return String(localized: "just now")
        }
        let time = date.formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute(.twoDigits))
        if Calendar.current.isDate(date, inSameDayAs: now) {
            return String(localized: "today \(time)")
        }
        return date.formatted(date: .abbreviated, time: .shortened)
    }
}

/// Advanced › Node.js / Bun: the executable Runs use, and its version.
private struct ToolRow: View {
    let model: AppModel
    let tool: ToolState
    let toaster: Toaster

    private var name: String {
        switch tool.tool {
        case .node: "Node.js"
        case .bun: "Bun"
        }
    }

    var body: some View {
        LabeledContent {
            HStack(spacing: 8) {
                if let version = tool.version {
                    Text(verbatim: version)
                        .foregroundStyle(.secondary)
                }
                if tool.isCustom {
                    Button("Use PATH") {
                        if let message = model.setToolPath(tool.tool, to: nil) {
                            toaster.show(message)
                        }
                    }
                }
                Button("Change…", action: choose)
            }
        } label: {
            Text(verbatim: name)
            if let path = tool.path {
                Text(path.abbreviatingWithTilde)
                    .font(.caption.monospaced())
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .help(path)
            } else {
                Text("Not found")
                    .foregroundStyle(Color(nsColor: .systemRed))
            }
        }
    }

    private func choose() {
        Task {
            let panel = NSOpenPanel()
            panel.canChooseFiles = true
            panel.canChooseDirectories = false
            panel.allowsMultipleSelection = false
            panel.showsHiddenFiles = true
            panel.treatsFilePackagesAsDirectories = true
            panel.prompt = String(localized: "Use")
            panel.message = String(localized: "Choose the \(name) executable.")
            if let path = tool.path {
                panel.directoryURL = URL(filePath: path).deletingLastPathComponent()
            }
            guard let url = await Panels.run(panel, in: NSApp.keyWindow) else { return }
            if let message = model.setToolPath(tool.tool, to: url) {
                toaster.show(message)
            }
        }
    }
}

#Preview {
    SettingsView(
        model: AppModel(dataDirectory: URL.temporaryDirectory.appending(path: "BunodePreview")),
        navigation: SettingsNavigation()
    )
}
