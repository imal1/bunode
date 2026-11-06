import SwiftUI

/// The Service view (design-handoff §2.2): status and Start/Stop at the top,
/// then the Service's settings and its Logs.
struct ServiceView: View {
    let model: AppModel
    let serviceID: UInt64
    let router: PopoverRouter
    let toaster: Toaster

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
                    }
                }
            }
        }
    }
}

/// `Running · Run #14 · :3000` with a small status dot.
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
