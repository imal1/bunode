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
                AdvancedPane(model: model)
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

    var body: some View {
        Form {
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
            }
        }
        .formStyle(.grouped)
        .controlSize(.small)
    }
}

#Preview {
    SettingsView(
        model: AppModel(dataDirectory: URL.temporaryDirectory.appending(path: "BunodePreview")),
        navigation: SettingsNavigation()
    )
}
