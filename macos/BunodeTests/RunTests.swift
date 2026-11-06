import Foundation
import Testing
@testable import Bunode

/// Starts and stops real Runs through the bundled bunode-runner.
@MainActor
struct RunTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    private func model(withService command: String) async throws -> (AppModel, UInt64) {
        let model = AppModel(dataDirectory: data.url)
        #expect(AppModel.runnerURL != nil, "bunode-runner is bundled")
        _ = await model.addProject(at: code.folder("app"))
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "sleeper", command: command, folder: nil).get()
        return (model, service.id)
    }

    private func waitFor(_ model: AppModel, _ id: UInt64, _ status: RunStatus) async throws {
        let deadline = Date.now.addingTimeInterval(15)
        while model.service(id)?.status != status {
            if Date.now > deadline {
                Issue.record("timed out waiting for \(status); now \(String(describing: model.service(id)?.status))")
                return
            }
            try await Task.sleep(for: .milliseconds(50))
            model.refresh()
        }
    }

    @Test func startsAndStopsARun() async throws {
        let (model, id) = try await model(withService: "sleep 30")

        model.start(id)
        // Feedback is immediate, before the Supervisor answers.
        #expect(model.status(of: try #require(model.service(id))) == .starting)
        try await waitFor(model, id, .running)
        #expect(model.statusSummary.running == 1)
        #expect(model.service(id)?.runNumber == 1)

        model.stop(id)
        try await waitFor(model, id, .stopped)
        #expect(model.statusSummary.running == 0)
    }

    @Test func runningCountCanBeHidden() async throws {
        let model = AppModel(dataDirectory: data.url)
        #expect(model.statusSummary.showsCount)
        model.setShowRunningCount(false)
        #expect(!model.statusSummary.showsCount)
        #expect(!AppModel(dataDirectory: data.url).snapshot.settings.showRunningCount)
    }
}
