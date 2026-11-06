import Foundation
import Testing
@testable import Bunode

@MainActor
struct EnvironmentTests {
    let data = TemporaryDirectory()

    @Test func readsTheLoginShellEnvironment() async throws {
        let model = AppModel(dataDirectory: data.url)
        let deadline = Date.now.addingTimeInterval(20)
        while model.snapshot.environment.loadedAt == nil || model.snapshot.environment.isLoading {
            guard Date.now < deadline else {
                Issue.record("the environment never loaded")
                return
            }
            try await Task.sleep(for: .milliseconds(100))
            model.refresh()
        }
        let environment = model.snapshot.environment
        #expect(environment.variableCount > 0)
        #expect(environment.tools.map(\.tool) == [.node, .bun])
    }

    @Test func describesWhenTheEnvironmentWasRead() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        #expect(EnvironmentTime.describe(now.addingTimeInterval(-10), now: now) == "just now")
        #expect(EnvironmentTime.describe(now.addingTimeInterval(-3600), now: now).hasPrefix("today "))
    }

    @Test func rejectsAToolThatIsNotExecutable() throws {
        let model = AppModel(dataDirectory: data.url)
        let file = data.url.appending(path: "node")
        try "not a program".write(to: file, atomically: true, encoding: .utf8)
        let message = model.setToolPath(.node, to: file)
        #expect(message?.contains("is not an executable file") == true)
    }
}
