import Foundation
import Testing
@testable import Bunode

/// The Service view's Start with Bunode (#37). Each model here is one
/// launch of Bunode on a data directory of its own.
@MainActor
struct StartWithBunodeTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    private func waitFor(_ model: AppModel, _ id: UInt64, _ status: RunStatus) async {
        let deadline = Date.now.addingTimeInterval(20)
        while model.service(id)?.status != status {
            if Date.now > deadline {
                Issue.record("timed out waiting for \(status); now \(String(describing: model.service(id)?.status))")
                return
            }
            try? await Task.sleep(for: .milliseconds(50))
            model.refresh()
        }
    }

    @Test func isOffForANewServiceAndKeptOnceChosen() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("api"))
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "dev", command: "sleep 30", folder: nil).get()
        #expect(!service.startWithBunode)

        #expect(model.setStartWithBunode(service.id, true) == nil)
        let chosen = try #require(model.service(service.id))
        #expect(chosen.startWithBunode)
        // Nothing starts while Bunode is running, and there is nothing for
        // a Run to apply.
        #expect(chosen.status == .stopped)
        #expect(!chosen.hasPendingConfig)
        #expect(chosen.command == "sleep 30")

        #expect(model.setStartWithBunode(service.id + 100, true) == "This Service is no longer in Bunode.")
    }

    @Test func startsTheChosenServicesWhenBunodeStarts() async throws {
        var chosen: UInt64 = 0
        var other: UInt64 = 0
        do {
            let model = AppModel(dataDirectory: data.url)
            _ = await model.addProject(at: code.folder("api"))
            let project = try #require(model.snapshot.projects.first)
            chosen = try await model.addService(to: project.id, name: "dev", command: "sleep 30", folder: nil).get().id
            other = try await model.addService(to: project.id, name: "worker", command: "sleep 31", folder: nil).get().id
            #expect(model.setStartWithBunode(chosen, true) == nil)
        }

        let relaunched = AppModel(dataDirectory: data.url)
        await waitFor(relaunched, chosen, .running)
        #expect(relaunched.service(chosen)?.runNumber == 1)
        #expect(relaunched.service(other)?.status == .stopped)
        #expect(relaunched.service(other)?.runNumber == nil)

        // Stopped by hand, it is not started again in this launch.
        relaunched.stop(chosen)
        await waitFor(relaunched, chosen, .stopped)
        try await Task.sleep(for: .milliseconds(500))
        relaunched.refresh()
        #expect(relaunched.service(chosen)?.status == .stopped)
        #expect(relaunched.service(chosen)?.runNumber == 1)
    }

    @Test func wordsAPortThatWasTaken() {
        let unknown = PortConflict(port: 3000, owner: nil, suggestion: 3001)
        #expect(StartupConflictView.text(unknown) == "Didn’t start with Bunode: port 3000 was used by another program.")

        let owner = PortConflictOwner(pid: 7, command: "astro dev", source: "Terminal", serviceId: nil, agentServer: false)
        let known = PortConflict(port: 4321, owner: owner, suggestion: nil)
        #expect(StartupConflictView.text(known) == "Didn’t start with Bunode: port 4321 was used by “astro dev” from Terminal.")
    }
}
