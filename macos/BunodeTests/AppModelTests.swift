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

@MainActor
struct AddServiceTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func offersScriptsAndAddsAService() async throws {
        let model = AppModel(dataDirectory: data.url)
        let acme = code.folder("acme", files: [
            "package.json": #"{"scripts": {"dev": "vite", "storybook": "storybook dev"}}"#,
            "pnpm-lock.yaml": "",
        ])
        _ = await model.addProject(at: acme)
        let project = try #require(model.snapshot.projects.first)

        let scripts = try #require(await model.projectScripts(project.id))
        #expect(scripts.hasPackageJson)
        #expect(scripts.groups.first?.scripts.map(\.command) == ["pnpm dev", "pnpm storybook"])

        let added = try await model.addService(to: project.id, name: "storybook", command: "pnpm storybook", folder: nil).get()
        #expect(added.name == "storybook")
        #expect(model.project(project.id)?.services.map(\.name) == ["storybook"])

        let used = try #require(await model.projectScripts(project.id))
        #expect(used.groups.first?.scripts.map(\.used) == [false, true])

        let duplicate = await model.addService(to: project.id, name: "Storybook", command: "x", folder: nil)
        guard case .failure(let error) = duplicate else {
            Issue.record("a duplicate name was accepted")
            return
        }
        #expect(error.userMessage == "Name already used in acme")
    }

    @Test func folderWithoutPackageJsonHasNoScripts() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("plain"))
        let project = try #require(model.snapshot.projects.first)
        let scripts = try #require(await model.projectScripts(project.id))
        #expect(!scripts.hasPackageJson)
    }
}

@MainActor
struct PopoverRouterTests {
    @Test func startsAtHomeAndGoesBack() {
        let router = PopoverRouter()
        #expect(router.screen == .home)
        router.push(.addService(projectID: 7))
        #expect(router.screen == .addService(projectID: 7))
        router.back()
        #expect(router.screen == .home)
        router.back()
        #expect(router.screen == .home)
    }

    @Test func resetReturnsToHome() {
        let router = PopoverRouter()
        router.push(.addService(projectID: 1))
        router.reset()
        #expect(router.screen == .home)
        #expect(router.stack == [.home])
    }
}
