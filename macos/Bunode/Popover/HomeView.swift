import SwiftUI

/// Home (design-handoff §2.2): every Project, in registration order, with
/// its Services in creation order.
struct HomeView: View {
    let model: AppModel
    let router: PopoverRouter
    let openSettings: (SettingsPane?, Bool) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if model.snapshot.projects.isEmpty {
                emptyState
            } else {
                PopoverScroll {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(model.snapshot.projects, id: \.id) { project in
                            ProjectSection(project: project, router: router)
                        }
                    }
                }
            }
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text("Bunode")
                .font(.body.weight(.semibold))
                .frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityAddTraits(.isHeader)
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
    let project: ProjectState
    let router: PopoverRouter

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 4) {
                Text(project.name)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .accessibilityAddTraits(.isHeader)
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
                ServiceRow(project: project, service: service)
            }
        }
    }
}

struct ServiceRow: View {
    let project: ProjectState
    let service: ServiceState

    private var displayName: String {
        "\(project.name) › \(service.name)"
    }

    var body: some View {
        HStack(spacing: 8) {
            StatusDot(style: .stopped)
            Text(service.name)
                .lineLimit(1)
                .truncationMode(.tail)
            Spacer(minLength: 4)
            if let port = service.port {
                Text(verbatim: ":\(port)")
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
            }
        }
        .padding(EdgeInsets(top: 0, leading: 10, bottom: 0, trailing: 4))
        .frame(height: 32)
        .contentShape(Rectangle())
        .hoverHighlight()
        .help(displayName)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityLabel)
    }

    private var accessibilityLabel: String {
        var label = String(localized: "\(displayName), Stopped")
        if let port = service.port {
            label += String(localized: ", port \(String(port))")
        }
        return label
    }
}
