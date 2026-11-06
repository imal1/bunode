import AppKit
import SwiftUI

/// Logs (design-handoff §2.2): one Service's current or last Run, live.
struct LogsView: View {
    let model: AppModel
    let serviceID: UInt64
    let router: PopoverRouter
    let toaster: Toaster

    @State private var feed = LogFeed()

    private var service: ServiceState? { model.service(serviceID) }
    private var project: ProjectState? { service.flatMap { model.project($0.projectId) } }

    var body: some View {
        if let service, let project {
            let displayName = "\(project.name) › \(service.name)"
            VStack(alignment: .leading, spacing: 0) {
                TopBar(title: title(for: service), backLabel: String(localized: "Back to \(displayName)"), onBack: router.back) {
                    Text(displayName)
                } trailing: {
                    trailingButton(service)
                }
                ZStack {
                    LogTableView(feed: feed, accessibilityName: String(localized: "Logs of \(displayName)"))
                    if service.logCount == 0 {
                        Text("No output yet.")
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
            .onAppear(perform: sync)
            .onChange(of: service.logFirst, sync)
            .onChange(of: service.logCount, sync)
        }
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
        if !status.isLive {
            Button("Start") { model.start(service.id) }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
        }
    }

    private func footer(_ service: ServiceState) -> some View {
        HStack(spacing: 4) {
            HStack(spacing: 0) {
                Text("\(Int(service.logCount)) lines")
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
        }
        .buttonStyle(.borderless)
        .controlSize(.small)
        .padding(EdgeInsets(top: 8, leading: 2, bottom: 2, trailing: 2))
    }

    private func sync() {
        guard let service = model.service(serviceID) else { return }
        feed.sync(first: service.logFirst, count: service.logCount) { from, max in
            model.logLines(serviceID, from: from, max: max)
        }
    }

    private func copyAll() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(LogPalette.plainText(of: feed.mirror.lines), forType: .string)
    }
}
