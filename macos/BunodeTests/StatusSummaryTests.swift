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

struct PortDisplayTests {
    private func service(port: UInt16?, live: [UInt16]) -> ServiceState {
        ServiceState(
            id: 1, projectId: 1, name: "dev", displayName: "dev", command: "x", folder: nil,
            port: port, runThroughShell: false, status: .running, runNumber: 1, pid: 10,
            lastExit: nil, logFirst: 0, logCount: 0, logDropped: 0, logMatchFirst: 0,
            logMatchCount: 0, logFilterGeneration: 0, hasPendingConfig: false, endedBy: nil,
            livePorts: live
        )
    }

    @Test func showsOnlyThePortsARunningRunListensOn() {
        #expect(service(port: 3000, live: [5173, 5174]).shownPorts(status: .running) == [5173, 5174])
        #expect(service(port: 3000, live: []).shownPorts(status: .running).isEmpty)
        #expect(service(port: 3000, live: [5173]).shownPorts(status: .stopped).isEmpty)
        #expect(service(port: 3000, live: [5173]).shownPorts(status: .stopping).isEmpty)
    }
}

struct CommandPartsTests {
    @Test func splitsOffAPackageManagerRun() {
        let parts = ServiceState.commandParts("bun run dev")
        #expect(parts.prefix == "bun run")
        #expect(parts.rest == "dev")
    }

    @Test(arguments: [
        ("npm run dev", "npm run", "dev"),
        ("pnpm run build:web", "pnpm run", "build:web"),
        ("yarn run test --watch", "yarn run", "test --watch"),
        ("pnpm --filter ui storybook", "pnpm --filter", "ui storybook"),
        ("pnpm storybook", "pnpm", "storybook"),
        ("yarn dev", "yarn", "dev"),
        ("bun index.ts", "bun", "index.ts"),
        ("npm test", "npm", "test"),
    ])
    func splitsEachPackageManagerForm(command: String, prefix: String, rest: String) {
        let parts = ServiceState.commandParts(command)
        #expect(parts.prefix == prefix)
        #expect(parts.rest == rest)
    }

    @Test(arguments: ["node server.js", "bunx vite", "npm", "sleep 30"])
    func leavesOtherCommandsWhole(command: String) {
        let parts = ServiceState.commandParts(command)
        #expect(parts.prefix == nil)
        #expect(parts.rest == command)
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
