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

    @Test func namesAServiceAfterItsScript() {
        #expect(AddServiceView.serviceName(script: "storybook", command: "pnpm storybook") == "storybook")
    }

    @Test func namesAHandTypedCommandAfterItsLastWord() {
        #expect(AddServiceView.serviceName(script: nil, command: "  bun run  worker ") == "worker")
        #expect(AddServiceView.serviceName(script: nil, command: "   ").isEmpty)
    }

    /// The Supervisor counts Unicode scalars, so a flag emoji counts as two.
    @Test func cutsALongNameToWhatTheSupervisorAccepts() {
        let forty = String(repeating: "a", count: 40)
        #expect(AddServiceView.serviceName(script: forty + "bc", command: "x") == forty)
        #expect(AddServiceView.serviceName(script: nil, command: "bun run \(forty)bc") == forty)
        let flags = String(repeating: "🇨🇳", count: 21)
        #expect(AddServiceView.serviceName(script: flags, command: "x").unicodeScalars.count == 40)
    }

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

@MainActor
struct EditServiceTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func savesEditsAndRejectsBadOnes() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("api"))
        _ = code.folder("api/server")
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "dev", command: "bun run dev", folder: nil).get()

        #expect(model.updateService(service.id, ServiceEdit(command: "node 'x", folder: nil, port: nil, runThroughShell: false)) == "The command has an unmatched quote.")
        #expect(model.updateService(service.id, ServiceEdit(command: "x", folder: "..", port: nil, runThroughShell: false)) == "Choose the Project's folder or a folder inside it.")
        #expect(model.updateService(service.id, ServiceEdit(command: "node index.js", folder: "server", port: 4000, runThroughShell: true)) == nil)

        let saved = try #require(model.service(service.id))
        #expect(saved.command == "node index.js")
        #expect(saved.folder == "server")
        #expect(saved.port == 4000)
        #expect(saved.runThroughShell)
    }

    @Test func deletesAndSaysTheFilesStay() async throws {
        let model = AppModel(dataDirectory: data.url)
        let folder = code.folder("api", files: ["package.json": "{}"])
        _ = await model.addProject(at: folder)
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "worker", command: "sleep 30", folder: nil).get()
        model.start(service.id)

        let message = await model.deleteService(service.id)
        #expect(message.hasPrefix("Deleted api › worker. Files in "))
        #expect(model.service(service.id) == nil)
        #expect(FileManager.default.fileExists(atPath: folder.appending(path: "package.json").path(percentEncoded: false)))
    }
}

@MainActor
struct MonorepoTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func offersPackageScriptsAndNamesThemByFolder() async throws {
        let model = AppModel(dataDirectory: data.url)
        let acme = code.folder("acme", files: [
            "package.json": #"{"workspaces": ["apps/*"], "scripts": {"dev": "turbo dev"}}"#,
        ])
        _ = code.folder("acme/apps/web", files: ["package.json": #"{"name": "@acme/web", "scripts": {"dev": "vite"}}"#])
        _ = await model.addProject(at: acme)
        let project = try #require(model.snapshot.projects.first)

        let scripts = try #require(await model.projectScripts(project.id))
        #expect(scripts.groups.map(\.folder) == [nil, "apps/web"])
        #expect(scripts.groups[1].packageName == "@acme/web")

        let root = try await model.addService(to: project.id, name: "dev", command: "npm run dev", folder: nil).get()
        let web = try await model.addService(to: project.id, name: "dev", command: "npm run dev", folder: "apps/web").get()
        #expect(root.displayName == "dev")
        #expect(web.displayName == "web › dev")
    }
}

@MainActor
struct ManageProjectTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func picksAPackageManagerAndRemovesProjects() async throws {
        let model = AppModel(dataDirectory: data.url)
        let api = code.folder("api", files: ["package.json": "{}", "yarn.lock": "", "package-lock.json": ""])
        _ = await model.addProject(at: api)
        let project = try #require(model.snapshot.projects.first)
        #expect(project.severalLockfiles)

        model.setPackageManager(project.id, .npm)
        #expect(model.project(project.id)?.severalLockfiles == false)
        #expect(model.project(project.id)?.packageManagerChoice == .npm)

        _ = try await model.addService(to: project.id, name: "dev", command: "sleep 30", folder: nil).get()
        let current = try #require(model.project(project.id))
        #expect(AppModel.removalMessage(for: current, running: 1) == "Its 1 Service leaves Bunode. Files in \(current.path.abbreviatingWithTilde) stay on disk. 1 running Service will be stopped first.")

        let message = await model.removeProject(project.id)
        #expect(message == "Removed api. Files untouched.")
        #expect(model.snapshot.projects.isEmpty)
        #expect(FileManager.default.fileExists(atPath: api.appending(path: "package.json").path(percentEncoded: false)))
    }

    @Test func startingInAMissingFolderAsksToLocateIt() async throws {
        let model = AppModel(dataDirectory: data.url)
        let api = code.folder("api")
        _ = await model.addProject(at: api)
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "dev", command: "sleep 30", folder: nil).get()

        let moved = code.url.appending(path: "api-moved")
        try FileManager.default.moveItem(at: api, to: moved)
        model.refresh()
        #expect(model.project(project.id)?.isMissing == true)

        model.start(service.id)
        #expect(model.rowNotice == .missingFolder(serviceID: service.id, projectID: project.id))
        #expect(model.service(service.id)?.status == .stopped)

        #expect(model.relocateProject(project.id, to: moved) == nil)
        #expect(model.rowNotice == nil)
        #expect(model.project(project.id)?.isMissing == false)
    }
}
