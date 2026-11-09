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
    /// The one warning bar shown under a Service row (design-handoff §5.3, §7).
    private(set) var rowNotice: RowNotice?

    enum RowNotice: Equatable {
        /// Start was refused because the Project's folder is gone.
        case missingFolder(serviceID: UInt64, projectID: UInt64)
        /// Start was refused because the Service's Port is taken.
        case portConflict(serviceID: UInt64, port: UInt16, owner: PortConflictOwner?, suggestion: UInt16?)

        var serviceID: UInt64 {
            switch self {
            case .missingFolder(let serviceID, _): serviceID
            case .portConflict(let serviceID, _, _, _): serviceID
            }
        }
    }

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

    /// `~/Library/Logs/Bunode`: the Supervisor's and the runners' logs.
    static var defaultDiagnosticsDirectory: URL {
        URL.libraryDirectory.appending(path: "Logs/Bunode", directoryHint: .isDirectory)
    }

    init(dataDirectory: URL, runner: URL? = AppModel.runnerURL, diagnostics: URL? = nil) {
        let listener = CoreListenerBridge()
        core = BunodeCore(
            options: CoreOptions(
                dataDir: dataDirectory.path(percentEncoded: false),
                runnerPath: runner?.path(percentEncoded: false) ?? "",
                diagnosticsDir: diagnostics?.path(percentEncoded: false)
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
        StatusSummary(services: services, status: status(of:), showsCount: snapshot.settings.showRunningCount)
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

    /// Saves the Service view's settings. Returns why they couldn't be saved.
    func updateService(_ serviceID: UInt64, _ edit: ServiceEdit) -> String? {
        do {
            try core.updateService(serviceId: serviceID, edit: edit)
            refresh()
            return nil
        } catch {
            return error.userMessage
        }
    }

    /// Deletes a Service: its Run is stopped first, then the configuration
    /// goes. Returns the toast once the Supervisor has removed it.
    func deleteService(_ serviceID: UInt64) async -> String {
        guard let service = service(serviceID), let project = project(service.projectId) else {
            return String(localized: "This Service is no longer in Bunode.")
        }
        let core = self.core
        let result = await Task.detached { Result { try core.deleteService(serviceId: serviceID) } }.value
        if case .failure(let error) = result {
            return error.userMessage
        }
        let deadline = Date.now.addingTimeInterval(responseTimeout + 10)
        while self.service(serviceID) != nil, Date.now < deadline {
            try? await Task.sleep(for: .milliseconds(100))
            refresh()
        }
        return String(localized: "Deleted \(project.name) › \(service.displayName). Files in \(project.path.abbreviatingWithTilde) are untouched.")
    }

    // MARK: - Runs

    func start(_ serviceID: UInt64) {
        if let service = service(serviceID), let project = project(service.projectId), project.isMissing {
            rowNotice = .missingFolder(serviceID: serviceID, projectID: project.id)
            return
        }
        if rowNotice?.serviceID == serviceID {
            rowNotice = nil
        }
        launch(serviceID) { core in try? core.start(serviceId: serviceID) }
    }

    /// Port conflict › Use :N: this one Run gets `PORT=N`.
    func start(_ serviceID: UInt64, port: UInt16) {
        rowNotice = nil
        launch(serviceID) { core in try? core.startWithPort(serviceId: serviceID, port: port) }
    }

    /// Starts with feedback at once; a taken port shows the conflict bar,
    /// replacing any other (one at a time, design-handoff §5.3).
    private func launch(_ serviceID: UInt64, _ start: @escaping @Sendable (BunodeCore) -> StartOutcome?) {
        optimistic[serviceID] = .starting
        let core = self.core
        Task {
            let outcome = await Task.detached { start(core) }.value
            optimistic[serviceID] = nil
            if case .portInUse(let port, let owner, let suggestion) = outcome {
                rowNotice = .portConflict(serviceID: serviceID, port: port, owner: owner, suggestion: suggestion)
            }
            refresh()
        }
    }

    /// Whether something listens on `port`.
    func portInUse(_ port: UInt16) async -> Bool {
        let core = self.core
        return await Task.detached { core.portInUse(port: port) }.value
    }

    /// Stops the Run completely, then starts a new one.
    func restart(_ serviceID: UInt64) {
        send(serviceID, showing: .restarting) { core in
            try? core.restart(serviceId: serviceID)
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

    /// Reads the login shell's environment again; new Runs get it.
    func reloadEnvironment() {
        core.reloadEnvironment()
        refresh()
    }

    /// Advanced › Node.js / Bun › Change…; `nil` goes back to PATH.
    func setToolPath(_ tool: Tool, to url: URL?) -> String? {
        do {
            try core.setToolPath(tool: tool, path: url?.path(percentEncoded: false))
            refresh()
            return nil
        } catch {
            return error.userMessage
        }
    }

    // MARK: - Logs

    func logLines(_ serviceID: UInt64, from: UInt64, max: UInt32) -> [LogLine] {
        core.logLines(serviceId: serviceID, from: from, max: max)
    }

    func logMatchingLines(_ serviceID: UInt64, from: UInt64, max: UInt32) -> [LogLine] {
        core.logMatchingLines(serviceId: serviceID, from: from, max: max)
    }

    /// Logs › Search; filtering happens in the core, off the main thread.
    func setLogFilter(_ serviceID: UInt64, _ query: String) {
        core.setLogFilter(serviceId: serviceID, query: query)
    }

    /// Logs › Export…: the full log of the current or last Run.
    func exportLog(_ serviceID: UInt64, to url: URL) async -> String? {
        let core = self.core
        let path = url.path(percentEncoded: false)
        let result = await Task.detached {
            Result { try core.exportLog(serviceId: serviceID, destination: path) }
        }.value
        if case .failure(let error) = result {
            return error.userMessage
        }
        return nil
    }

    /// `api-dev-run14.log`
    nonisolated static func exportFileName(project: String, service: String, run: UInt64?) -> String {
        let safe = { (name: String) in
            name.map { "/:\\".contains($0) || $0.isWhitespace ? "-" : String($0) }.joined()
        }
        let run = run.map { "-run\($0)" } ?? ""
        return "\(safe(project))-\(safe(service))\(run).log"
    }

    func clearLogs(_ serviceID: UInt64) {
        core.clearLogs(serviceId: serviceID)
        refresh()
    }

    /// Advanced › Log lines kept.
    func setLogLinesKept(_ lines: UInt32) {
        try? core.setLogLinesKept(lines: lines)
        refresh()
    }

    /// Advanced › Force stop after, in seconds.
    func setForceStopAfter(_ seconds: UInt32) {
        try? core.setForceStopAfter(seconds: seconds)
        refresh()
    }

    /// Services with a live Run.
    var liveCount: Int {
        services.count(where: { status(of: $0).isLive })
    }

    /// Quit: stops every Run and waits until the Supervisor reports them
    /// all ended, or the Force stop timeout and then some has passed.
    func stopAllAndWait() async {
        let core = self.core
        _ = await Task.detached { core.stopAll() }.value
        let deadline = Date.now.addingTimeInterval(responseTimeout + 5)
        refresh()
        while services.contains(where: { $0.status.isLive }), Date.now < deadline {
            try? await Task.sleep(for: .milliseconds(100))
            refresh()
        }
    }

    // MARK: - Discovered Processes

    /// Looks for other Node and Bun processes now.
    func rescan() {
        core.rescan()
    }

    /// Other processes › Send SIGTERM. Returns a problem, if any.
    func stopDiscovered(_ process: DiscoveredProcess) -> String? {
        do {
            try core.stopDiscovered(pid: process.pid, start: process.start)
            refresh()
            return nil
        } catch {
            return error.userMessage
        }
    }

    func forceKillDiscovered(_ process: DiscoveredProcess) {
        try? core.forceKillDiscovered(pid: process.pid, start: process.start)
        refresh()
    }

    /// Whether this exact process is still listed.
    func isStillRunning(_ process: DiscoveredProcess) -> Bool {
        snapshot.discovered.contains { $0.pid == process.pid && $0.start == process.start }
    }

    func dismissNotice() {
        rowNotice = nil
    }

    // MARK: - Projects

    /// Settings › Projects › package manager; `nil` is Auto.
    func setPackageManager(_ projectID: UInt64, _ manager: PackageManager?) {
        try? core.setPackageManager(projectId: projectID, manager: manager)
        refresh()
    }

    /// Points a Project whose folder moved at its new place. Returns a
    /// problem to show, if any.
    func relocateProject(_ projectID: UInt64, to url: URL) -> String? {
        do {
            try core.relocateProject(projectId: projectID, path: url.path(percentEncoded: false))
            refresh()
            if case .missingFolder(_, projectID) = rowNotice {
                rowNotice = nil
            }
            return nil
        } catch {
            return error.userMessage
        }
    }

    /// Removes a Project; its Runs are stopped first. Returns the toast once
    /// the Supervisor has removed it.
    func removeProject(_ projectID: UInt64) async -> String {
        guard let project = project(projectID) else {
            return String(localized: "This Project is no longer in Bunode.")
        }
        let core = self.core
        let result = await Task.detached { Result { try core.removeProject(projectId: projectID) } }.value
        if case .failure(let error) = result {
            return error.userMessage
        }
        let deadline = Date.now.addingTimeInterval(responseTimeout + 10)
        while self.project(projectID) != nil, Date.now < deadline {
            try? await Task.sleep(for: .milliseconds(100))
            refresh()
        }
        return String(localized: "Removed \(project.name). Files untouched.")
    }

    /// Body of the Remove alert: what leaves, what stays, what stops.
    static func removalMessage(for project: ProjectState, running: Int) -> String {
        var sentences = [
            String(localized: "Its \(project.services.count) Services leave Bunode."),
            String(localized: "Files in \(project.path.abbreviatingWithTilde) stay on disk."),
        ]
        if running > 0 {
            sentences.append(String(localized: "\(running) running Services will be stopped first."))
        }
        return sentences.joined(separator: " ")
    }

    /// The Services of a Project with a live Run.
    func runningCount(in project: ProjectState) -> Int {
        project.services.count(where: { status(of: $0).isLive })
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
