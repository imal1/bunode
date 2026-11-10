import Testing
@testable import Bunode

struct StatusSummaryTests {
    @Test func idleWhenNothingRuns() {
        let summary = StatusSummary()
        #expect(summary.aggregate == .idle)
        #expect(summary.countText == nil)
        #expect(summary.accessibilityLabel == "Bunode, 0 running")
    }

    @Test func showsRunningCount() {
        let summary = StatusSummary(running: 2)
        #expect(summary.aggregate == .running)
        #expect(summary.countText == "2")
        #expect(summary.accessibilityLabel == "Bunode, 2 running")
    }

    @Test func crashTakesPrecedenceOverRunning() {
        let summary = StatusSummary(running: 2, crashed: 1)
        #expect(summary.aggregate == .error)
        #expect(summary.accessibilityLabel == "Bunode, 2 running, 1 crashed")
    }

    @Test func countCanBeTurnedOff() {
        let summary = StatusSummary(running: 2, showsCount: false)
        #expect(summary.countText == nil)
    }
}

private func service(command: String = "x", port: UInt16? = nil, live: [UInt16] = [], endedBy: RunEnd? = nil) -> ServiceState {
    ServiceState(
        id: 1, projectId: 1, name: "dev", displayName: "dev", command: command, folder: nil,
        port: port, runThroughShell: false, status: .running, runNumber: 1, pid: 10,
        lastExit: nil, logFirst: 0, logCount: 0, logDropped: 0, logMatchFirst: 0,
        logMatchCount: 0, logFilterGeneration: 0, hasPendingConfig: false, endedBy: endedBy,
        livePorts: live
    )
}

struct PortDisplayTests {
    @Test func showsOnlyThePortsARunningRunListensOn() {
        #expect(service(port: 3000, live: [5173, 5174]).shownPorts(status: .running) == [5173, 5174])
        #expect(service(port: 3000, live: []).shownPorts(status: .running).isEmpty)
        #expect(service(port: 3000, live: [5173]).shownPorts(status: .stopped).isEmpty)
        #expect(service(port: 3000, live: [5173]).shownPorts(status: .stopping).isEmpty)
    }
}

struct ServiceTitleTests {
    @Test(arguments: [
        ("bun run start", "start"),
        ("npm run build", "build"),
        ("pnpm run build:web", "build:web"),
        ("yarn run lint", "lint"),
        ("pnpm dev", "dev"),
        ("yarn serve", "serve"),
        ("npm test", "test"),
        ("bun run preview", "preview"),
        ("bun  run   dev-api", "dev-api"),
    ])
    func aPresetScriptReadsAsProjectAndScript(command: String, script: String) {
        #expect(service(command: command).title(projectName: "shop") == "shop \(script)")
        // In its Project's group the row leaves the name out.
        #expect(service(command: command).title(projectName: nil) == script)
    }

    @Test(arguments: [
        "bun run storybook",
        "npm run developer",
        "bun run dev --port 3000",
        "pnpm --filter web dev",
        "bun index.ts",
        "node server.js",
        "bunx vite",
        "npm",
    ])
    func anythingElseStaysWhole(command: String) {
        #expect(service(command: command).title(projectName: "shop") == command)
        #expect(service(command: command).title(projectName: nil) == command)
    }
}

@MainActor
struct RecommendedScriptTests {
    @Test func newServiceOffersOnlyTheBarePresets() {
        let names = ["analyze", "build", "lint", "lint-staged", "lint-staged:js", "lint:fix", "lint:js", "start", "start:w", "test", "tsc"]
        let options = names.map { ScriptOption(name: $0, command: "bun run \($0)", used: false) }
        let scripts = ProjectScripts(hasPackageJson: true, groups: [ScriptGroup(packageName: nil, folder: nil, scripts: options)])
        #expect(AddServiceView.recommended(scripts).groups.first?.scripts.map(\.name) == ["build", "lint", "start", "test"])
    }

    @Test(arguments: ["dev", "start", "serve", "preview", "build", "test", "lint"])
    func everyPresetIsRecommended(name: String) {
        #expect(ScriptOption(name: name, command: "bun run \(name)", used: false).isRecommended)
    }
}

struct ShortLivedServiceTests {
    @Test func aRunningShortLivedServiceIsNotCounted() {
        let services = [service(command: "bun run build"), service(command: "bun run start")]
        let summary = StatusSummary(services: services, status: { _ in .running }, showsCount: true)
        #expect(summary.running == 1)
        #expect(summary.countText == "1")
    }

    @Test func aFailedShortLivedServiceStillCountsAsCrashed() {
        let summary = StatusSummary(services: [service(command: "bun run build")], status: { _ in .crashed }, showsCount: true)
        #expect(summary.crashed == 1)
        #expect(summary.aggregate == .error)
    }

    @Test(arguments: ["bun run build", "npm test", "pnpm lint", "yarn run build:web", "bun run test:e2e"])
    func buildTestAndLintScriptsAreShortLived(command: String) {
        #expect(service(command: command).isShortLived)
    }

    @Test(arguments: [
        "bun run start", "pnpm dev", "npm run serve", "bun run preview", "bun run build:watch",
        "pnpm test-watch", "bun run test --watch", "bun run migrate", "node build.js", "build",
    ])
    func everythingElseIsLongLived(command: String) {
        #expect(!service(command: command).isShortLived)
    }

    @Test func atRestItShowsAHammerAndOffersRun() {
        let build = service(command: "bun run build")
        #expect(build.dotStyle(status: .stopped) == .stoppedShortLived)
        #expect(build.statusLabel(status: .stopped) == "Stopped")
        #expect(build.startTitle == "Run")
    }

    @Test func runningIsInProgressNotGreen() {
        let build = service(command: "bun run build")
        #expect(build.dotStyle(status: .running) == .busy)
        #expect(build.statusLabel(status: .running) == "Running…")
    }

    @Test func exitingByItselfIsDone() {
        let build = service(command: "bun run build", endedBy: .exited)
        #expect(build.dotStyle(status: .stopped) == .done)
        #expect(build.statusLabel(status: .stopped) == "Done")
    }

    @Test func beingStoppedIsNotDone() {
        let build = service(command: "bun run build", endedBy: .stoppedByUser)
        #expect(build.dotStyle(status: .stopped) == .stoppedShortLived)
        #expect(build.statusLabel(status: .stopped) == "Stopped")
    }

    @Test func aNonZeroExitReadsFailed() {
        let build = service(command: "bun run build", endedBy: .crashed)
        #expect(build.dotStyle(status: .crashed) == .failed)
        #expect(build.statusLabel(status: .crashed) == "Failed")
    }

    @Test func aLongLivedServiceKeepsItsReading() {
        let start = service(command: "bun run start", endedBy: .exited)
        #expect(start.dotStyle(status: .stopped) == .stopped)
        #expect(start.statusLabel(status: .stopped) == "Stopped")
        #expect(start.dotStyle(status: .running) == .running)
        #expect(start.statusLabel(status: .running) == "Running")
        #expect(start.statusLabel(status: .crashed) == "Crashed")
        #expect(start.startTitle == "Start")
    }
}

@MainActor
struct KeyboardTests {
    @Test func arrowsMoveBetweenRowsAndStopAtTheEnds() {
        let order: [UInt64] = [4, 7, 9]
        #expect(HomeView.focus(after: nil, in: order, step: 1) == 4)
        #expect(HomeView.focus(after: nil, in: order, step: -1) == 9)
        #expect(HomeView.focus(after: 4, in: order, step: 1) == 7)
        #expect(HomeView.focus(after: 9, in: order, step: 1) == 9)
        #expect(HomeView.focus(after: 4, in: order, step: -1) == 4)
        #expect(HomeView.focus(after: 99, in: order, step: 1) == 4)
        #expect(HomeView.focus(after: 4, in: [], step: 1) == nil)
    }

    @Test func openingThePopoverStartsAtHome() {
        let router = PopoverRouter()
        router.push(.logs(3))
        let before = router.openCount
        router.reset()
        #expect(router.screen == .home)
        #expect(router.openCount == before + 1)
    }
}
