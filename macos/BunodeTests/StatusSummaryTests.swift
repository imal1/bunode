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

    @Test func prefersTheLivePort() {
        let live = service(port: 3000, live: [5173]).shownPort(status: .running)
        #expect(live?.port == 5173)
        #expect(live?.isLive == true)
        let configured = service(port: 3000, live: []).shownPort(status: .running)
        #expect(configured?.port == 3000)
        #expect(configured?.isLive == false)
        let stopped = service(port: 3000, live: [5173]).shownPort(status: .stopped)
        #expect(stopped?.port == 3000)
        #expect(service(port: nil, live: []).shownPort(status: .running) == nil)
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
