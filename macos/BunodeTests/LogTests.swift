import Foundation
import Testing
@testable import Bunode

/// Logs open in Console: the Run's output is on disk as it comes.
@MainActor
struct RunLogTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func theRunsLogFileHoldsItsOutput() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("app"))
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "echo", command: "printf 'one\\ntwo\\n'", folder: nil).get()
        #expect(model.core.currentLogFile(serviceId: service.id) == nil)

        model.start(service.id)
        let deadline = Date.now.addingTimeInterval(15)
        while model.service(service.id)?.status.isLive != false || model.service(service.id)?.runNumber == nil {
            guard Date.now < deadline else { break }
            try await Task.sleep(for: .milliseconds(50))
            model.refresh()
        }
        let path = try #require(model.core.currentLogFile(serviceId: service.id))
        let text = try String(contentsOfFile: path, encoding: .utf8)
        #expect(text.contains(" bunode Run #1: printf"))
        #expect(text.contains(" out one"))
        #expect(text.contains(" out two"))
    }
}
