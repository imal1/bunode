import Foundation
import Observation

/// The app's view of the Supervisor: its latest snapshot, and the commands
/// views send to it. Views render the snapshot; they never work out Run
/// state on their own.
@MainActor
@Observable
final class AppModel {
    private(set) var snapshot: Snapshot

    @ObservationIgnored let core: BunodeCore
    @ObservationIgnored private var updates: Task<Void, Never>?

    /// `~/Library/Application Support/Bunode`
    static var defaultDataDirectory: URL {
        URL.applicationSupportDirectory.appending(path: "Bunode", directoryHint: .isDirectory)
    }

    init(dataDirectory: URL) {
        let listener = CoreListenerBridge()
        core = BunodeCore(
            options: CoreOptions(dataDir: dataDirectory.path(percentEncoded: false)),
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
    }

    func project(_ id: UInt64) -> ProjectState? {
        snapshot.projects.first { $0.id == id }
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
