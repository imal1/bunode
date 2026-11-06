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

@MainActor
struct FailureTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func missingCommandFailsAndTurnsTheIconRed() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("app"))
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "broken", command: "bunode-no-such-command", folder: nil).get()

        model.start(service.id)
        let deadline = Date.now.addingTimeInterval(15)
        while model.service(service.id)?.status != .failed, Date.now < deadline {
            try await Task.sleep(for: .milliseconds(50))
            model.refresh()
        }
        #expect(model.service(service.id)?.status == .failed)
        #expect(model.service(service.id)?.lastExit?.contains("bunode-no-such-command") == true)
        #expect(model.statusSummary.aggregate == .error)
        #expect(model.statusSummary.accessibilityLabel == "Bunode, 0 running, 1 crashed")
    }

    @Test func forceStopSettingPersists() {
        let model = AppModel(dataDirectory: data.url)
        #expect(model.snapshot.settings.forceStopAfterSecs == 5)
        #expect(model.responseTimeout == 10)
        model.setForceStopAfter(30)
        #expect(AppModel(dataDirectory: data.url).snapshot.settings.forceStopAfterSecs == 30)
    }
}

@MainActor
struct RestartTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func restartStartsANewRun() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("app"))
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "sleeper", command: "sleep 30", folder: nil).get()

        func waitFor(run: UInt64) async throws {
            let deadline = Date.now.addingTimeInterval(15)
            while !(model.service(service.id)?.status == .running && model.service(service.id)?.runNumber == run) {
                guard Date.now < deadline else {
                    Issue.record("Run #\(run) never ran")
                    return
                }
                try await Task.sleep(for: .milliseconds(50))
                model.refresh()
            }
        }

        model.start(service.id)
        try await waitFor(run: 1)
        model.restart(service.id)
        #expect(model.status(of: try #require(model.service(service.id))) == .restarting)
        try await waitFor(run: 2)
        model.stop(service.id)
    }
}

@MainActor
struct QuitTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func stopsEveryRunBeforeQuitting() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("app"))
        let project = try #require(model.snapshot.projects.first)
        var ids: [UInt64] = []
        for name in ["one", "two"] {
            ids.append(try await model.addService(to: project.id, name: name, command: "sleep 30", folder: nil).get().id)
        }
        ids.forEach(model.start)
        let deadline = Date.now.addingTimeInterval(15)
        while model.statusSummary.running < 2, Date.now < deadline {
            try await Task.sleep(for: .milliseconds(50))
            model.refresh()
        }
        #expect(model.liveCount == 2)

        await model.stopAllAndWait()
        #expect(model.liveCount == 0)
        #expect(ids.allSatisfy { model.service($0)?.endedBy == .bunodeQuit })
    }

    @Test func quitTextIsPluralized() {
        #expect(String(localized: "Stop \(1) Services and Quit") == "Stop 1 Service and Quit")
        #expect(String(localized: "\(2) Services are running. Quitting stops every Run.") == "2 Services are running. Quitting stops every Run.")
    }
}
