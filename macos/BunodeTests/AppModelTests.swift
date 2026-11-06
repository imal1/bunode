import Foundation
import Testing
@testable import Bunode

@MainActor
struct AppModelTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func addsAProjectAndReportsDuplicates() async {
        let model = AppModel(dataDirectory: data.url)
        let api = code.folder("api", files: ["package.json": "{}", "bun.lock": ""])

        let added = await model.addProject(at: api)
        #expect(added.hasPrefix("Added "))
        #expect(added.hasSuffix("(Bun). Add its Services from the menu bar."))
        #expect(model.snapshot.projects.map(\.name) == ["api"])

        let again = await model.addProject(at: api)
        #expect(again == "api is already added")
        #expect(model.snapshot.projects.count == 1)
    }

    @Test func projectsSurviveARelaunch() async {
        let api = code.folder("api")
        do {
            let model = AppModel(dataDirectory: data.url)
            _ = await model.addProject(at: api)
        }
        let relaunched = AppModel(dataDirectory: data.url)
        #expect(relaunched.snapshot.projects.map(\.name) == ["api"])
    }

    @Test func reportsAMissingFolder() async {
        let model = AppModel(dataDirectory: data.url)
        let message = await model.addProject(at: code.url.appending(path: "missing"))
        #expect(message.contains("is not a folder"))
    }
}
