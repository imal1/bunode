import Foundation
import Testing
@testable import Bunode

/// Stands in for macOS's login items, so no test touches the real ones.
@MainActor
private final class FakeLoginItem: LoginItemService {
    var status: LoginItemStatus
    /// What register and unregister throw, if anything.
    var failure: (any Error)?
    /// What a registration leaves behind: `.needsApproval` when the user
    /// has to allow it first.
    var statusAfterRegistering = LoginItemStatus.on
    private(set) var calls: [String] = []

    init(status: LoginItemStatus = .off) {
        self.status = status
    }

    func register() throws {
        calls.append("register")
        if let failure { throw failure }
        status = statusAfterRegistering
    }

    func unregister() throws {
        calls.append("unregister")
        if let failure { throw failure }
        status = .off
    }

    func openSystemSettings() {
        calls.append("openSystemSettings")
    }
}

private struct Refused: LocalizedError {
    var errorDescription: String? { "Operation not permitted" }
}

/// Settings › General › Open at login (#36).
@MainActor
struct LoginItemTests {
    @Test func isOffUntilTheUserTurnsItOn() {
        let system = FakeLoginItem()
        let model = LoginItemModel(service: system)
        #expect(!model.isOn)
        #expect(model.problem == nil)
        #expect(system.calls.isEmpty, "reading the status registers nothing")
    }

    @Test func showsWhatMacOSSays() {
        #expect(LoginItemModel(service: FakeLoginItem(status: .on)).isOn)
        #expect(!LoginItemModel(service: FakeLoginItem(status: .needsApproval)).isOn)
    }

    @Test func turnsOnAndOff() {
        let system = FakeLoginItem()
        let model = LoginItemModel(service: system)

        model.set(true)
        #expect(model.isOn)
        #expect(model.problem == nil)

        model.set(false)
        #expect(!model.isOn)
        #expect(model.problem == nil)
        #expect(system.calls == ["register", "unregister"])
    }

    @Test func aFailedRegistrationStaysOffWithTheReason() {
        let system = FakeLoginItem()
        system.failure = Refused()
        let model = LoginItemModel(service: system)

        model.set(true)
        #expect(!model.isOn)
        #expect(model.problem == "Couldn’t add Bunode to Login Items: Operation not permitted")
    }

    @Test func aFailedRemovalStaysOnWithTheReason() {
        let system = FakeLoginItem(status: .on)
        system.failure = Refused()
        let model = LoginItemModel(service: system)

        model.set(false)
        #expect(model.isOn)
        #expect(model.problem == "Couldn’t remove Bunode from Login Items: Operation not permitted")

        // The next change that works takes the reason away.
        system.failure = nil
        model.set(false)
        #expect(!model.isOn)
        #expect(model.problem == nil)
    }

    @Test func waitingForApprovalIsNotOn() {
        let system = FakeLoginItem()
        system.statusAfterRegistering = .needsApproval
        let model = LoginItemModel(service: system)

        model.set(true)
        #expect(!model.isOn)
        #expect(model.status == .needsApproval)

        model.openSystemSettings()
        #expect(system.calls == ["register", "openSystemSettings"])
    }

    @Test func picksUpAChangeMadeInSystemSettings() {
        let system = FakeLoginItem()
        let model = LoginItemModel(service: system)

        system.status = .on
        #expect(!model.isOn, "not until Settings is shown again")
        model.refresh()
        #expect(model.isOn)

        system.status = .off
        model.refresh()
        #expect(!model.isOn)
    }

    @Test func changingItLeavesServicesAlone() async throws {
        let data = TemporaryDirectory()
        let code = TemporaryDirectory()
        let app = AppModel(dataDirectory: data.url)
        _ = await app.addProject(at: code.folder("api"))
        let project = try #require(app.snapshot.projects.first)
        let service = try await app.addService(to: project.id, name: "dev", command: "sleep 30", folder: nil).get()
        #expect(app.setStartWithBunode(service.id, true) == nil)

        let model = LoginItemModel(service: FakeLoginItem())
        model.set(true)
        model.set(false)

        app.refresh()
        #expect(app.service(service.id)?.startWithBunode == true, "each Service's own choice")
        #expect(app.service(service.id)?.status == .stopped)
        #expect(app.service(service.id)?.runNumber == nil)
    }
}
