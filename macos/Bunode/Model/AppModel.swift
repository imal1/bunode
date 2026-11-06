import Foundation
import Observation

/// The app's view of the Supervisor: its latest snapshot, and the commands
/// views send to it. Views render the snapshot; they never work out Run
/// state on their own.
@MainActor
@Observable
final class AppModel {
    private(set) var snapshot: Snapshot
    /// Statuses shown before the Supervisor confirms a command, so a click
    /// gets visible feedback at once (design-handoff §5.1).
    private(set) var optimistic: [UInt64: RunStatus] = [:]
    /// Services the Supervisor has kept in Starting or Stopping for too long.
    private(set) var notResponding: Set<UInt64> = []

    @ObservationIgnored private var busyWatch = BusyWatch()
    @ObservationIgnored private var watchdog: Task<Void, Never>?

    @ObservationIgnored let core: BunodeCore
    @ObservationIgnored private var updates: Task<Void, Never>?

    /// `~/Library/Application Support/Bunode`
    static var defaultDataDirectory: URL {
        URL.applicationSupportDirectory.appending(path: "Bunode", directoryHint: .isDirectory)
    }

    /// The bundled `bunode-runner`, next to the app's executable.
    static var runnerURL: URL? {
        Bundle.main.url(forAuxiliaryExecutable: "bunode-runner")
    }

    init(dataDirectory: URL, runner: URL? = AppModel.runnerURL) {
        let listener = CoreListenerBridge()
        core = BunodeCore(
            options: CoreOptions(
                dataDir: dataDirectory.path(percentEncoded: false),
                runnerPath: runner?.path(percentEncoded: false) ?? ""
            ),
            listener: listener
        )
        snapshot = core.snapshot()
        updates = Task { [weak self] in
            for await _ in listener.changes {
                self?.refresh()
            }
        }
    }

    func refresh() {
        snapshot = core.snapshot()
        watchBusyServices()
    }

    /// Starting/Stopping for longer than Force stop + 5 s with no news.
    var responseTimeout: TimeInterval {
        TimeInterval(snapshot.settings.forceStopAfterSecs) + 5
    }

    private func watchBusyServices() {
        let busy = Set(services.filter { $0.status.isBusy }.map(\.id))
        busyWatch.update(busy: busy, now: .now)
        notResponding.formIntersection(busy)
        guard !busyWatch.isEmpty, watchdog == nil else { return }
        watchdog = Task { [weak self] in
            while let self, !self.busyWatch.isEmpty {
                let overdue = self.busyWatch.overdue(now: .now, timeout: self.responseTimeout)
                if overdue != self.notResponding {
                    self.notResponding = overdue
                }
                try? await Task.sleep(for: .seconds(1))
            }
            self?.watchdog = nil
        }
    }

    func project(_ id: UInt64) -> ProjectState? {
        snapshot.projects.first { $0.id == id }
    }

    func service(_ id: UInt64) -> ServiceState? {
        snapshot.projects.lazy.flatMap(\.services).first { $0.id == id }
    }

    /// The status to show: the Supervisor's, or the optimistic one while a
    /// command is on its way.
    func status(of service: ServiceState) -> RunStatus {
        optimistic[service.id] ?? service.status
    }

    var services: [ServiceState] {
        snapshot.projects.flatMap(\.services)
    }

    /// What the menu bar item shows.
    var statusSummary: StatusSummary {
        let statuses = services.map(status(of:))
        return StatusSummary(
            running: statuses.count(where: { $0 == .running }),
            crashed: statuses.count(where: { $0 == .crashed || $0 == .failed }),
            showsCount: snapshot.settings.showRunningCount
        )
    }

    // MARK: - Commands

    /// Registers a folder as a Project. Returns the message for the toast.
    func addProject(at url: URL) async -> String {
        let core = self.core
        let path = url.path(percentEncoded: false)
        let result = await Task.detached {
            Result { try core.addProject(path: path) }
        }.value
        refresh()
        switch result {
        case .success(.added(let project)):
            return String(localized: "Added \(project.path.abbreviatingWithTilde) (\(project.packageManager.displayName)). Add its Services from the menu bar.")
        case .success(.alreadyAdded(let project)):
            return String(localized: "\(project.name) is already added")
        case .failure(let error):
            return error.userMessage
        }
    }

    // MARK: - Runs

    func start(_ serviceID: UInt64) {
        send(serviceID, showing: .starting) { core in
            try? core.start(serviceId: serviceID)
        }
    }

    /// Stops the Run; when it is not responding, force-kills it instead.
    func stop(_ serviceID: UInt64) {
        if notResponding.contains(serviceID) {
            forceKill(serviceID)
            return
        }
        send(serviceID, showing: .stopping) { core in
            core.stop(serviceId: serviceID)
        }
    }

    func forceKill(_ serviceID: UInt64) {
        send(serviceID, showing: .stopping) { core in
            core.forceKill(serviceId: serviceID)
        }
    }

    /// Shows `status` at once, sends the command off the main thread, then
    /// switches back to what the Supervisor reports.
    private func send(_ serviceID: UInt64, showing status: RunStatus, _ command: @escaping @Sendable (BunodeCore) -> Void) {
        optimistic[serviceID] = status
        let core = self.core
        Task {
            await Task.detached { command(core) }.value
            optimistic[serviceID] = nil
            refresh()
        }
    }

    func setShowRunningCount(_ show: Bool) {
        try? core.setShowRunningCount(show: show)
        refresh()
    }

    /// Advanced › Force stop after, in seconds.
    func setForceStopAfter(_ seconds: UInt32) {
        try? core.setForceStopAfter(seconds: seconds)
        refresh()
    }

    /// The scripts the New Service view offers, read fresh from disk.
    func projectScripts(_ projectID: UInt64) async -> ProjectScripts? {
        let core = self.core
        return await Task.detached {
            try? core.projectScripts(projectId: projectID)
        }.value
    }

    func addService(to projectID: UInt64, name: String, command: String, folder: String?) async -> Result<ServiceState, any Error> {
        let core = self.core
        let service = NewService(name: name, command: command, folder: folder)
        let result = await Task.detached {
            Result { try core.addService(projectId: projectID, service: service) }
        }.value
        refresh()
        return result
    }
}
