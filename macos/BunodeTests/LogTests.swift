import Foundation
import Testing
@testable import Bunode

@MainActor
struct LogMirrorTests {
    /// A fake core log holding lines `first..<end`.
    final class FakeLog {
        var first: UInt64 = 0
        var end: UInt64 = 0
        var fetches = 0

        func fetch(_ from: UInt64, _ max: UInt32) -> [LogLine] {
            fetches += 1
            let start = Swift.max(from, first)
            guard start < end else { return [] }
            let stop = Swift.min(end, start + UInt64(max))
            return (start..<stop).map {
                LogLine(seq: $0, timeMs: 0, stream: .stdout, text: "line \($0)", spans: [])
            }
        }
    }

    @Test func appendsOnlyNewLinesAndDropsOldOnes() {
        let log = FakeLog()
        let mirror = LogMirror()
        log.end = 3
        #expect(mirror.sync(first: 0, count: 3, fetch: log.fetch) == .reload)
        #expect(mirror.lines.map(\.text) == ["line 0", "line 1", "line 2"])

        log.first = 1
        log.end = 5
        #expect(mirror.sync(first: 1, count: 4, fetch: log.fetch) == .update(removed: 1, appended: 2))
        #expect(mirror.lines.map(\.seq) == [1, 2, 3, 4])
        #expect(mirror.sync(first: 1, count: 4, fetch: log.fetch) == .none)
    }

    @Test func clearAndNewRunsReload() {
        let log = FakeLog()
        let mirror = LogMirror()
        log.end = 4
        _ = mirror.sync(first: 0, count: 4, fetch: log.fetch)
        // Clear: nothing shown, from line 4 on; every row leaves the top.
        log.first = 4
        #expect(mirror.sync(first: 4, count: 0, fetch: log.fetch) == .update(removed: 4, appended: 0))
        #expect(mirror.count == 0)
        // A new Run starts over at 0.
        log.first = 0
        log.end = 2
        #expect(mirror.sync(first: 0, count: 2, fetch: log.fetch) == .reload)
        #expect(mirror.count == 2)
    }

    @Test func bigBurstsReloadInChunks() {
        let log = FakeLog()
        let mirror = LogMirror()
        log.end = 1
        _ = mirror.sync(first: 0, count: 1, fetch: log.fetch)
        log.end = 12_001
        #expect(mirror.sync(first: 0, count: 12_001, fetch: log.fetch) == .reload)
        #expect(mirror.count == 12_001)
        #expect(mirror.lines.last?.seq == 12_000)
    }
}

@MainActor
struct LogPaletteTests {
    @Test func countsWrappedLines() {
        #expect(LogPalette.wrappedLineCount(of: "", columns: 10) == 1)
        #expect(LogPalette.wrappedLineCount(of: "0123456789", columns: 10) == 1)
        #expect(LogPalette.wrappedLineCount(of: "0123456789a", columns: 10) == 2)
        // Wide characters take two columns.
        #expect(LogPalette.wrappedLineCount(of: "日本語日本語", columns: 10) == 2)
    }

    @Test func mapsPaletteColors() {
        #expect(LogPalette.palette(16).redComponent == 0)
        #expect(LogPalette.palette(231).redComponent == 1)
        #expect(LogPalette.palette(255).redComponent > 0.9)
    }

    @Test func pluralizesLines() {
        #expect(String(localized: "\(1) lines") == "1 line")
        #expect(String(localized: "\(10_000) lines") == "10,000 lines")
        #expect(String(localized: "\(1) matches") == "1 match")
    }
}

@MainActor
struct RunLogTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func collectsTheRunsOutput() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("app"))
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "echo", command: "printf 'one\\ntwo\\n'", folder: nil).get()

        model.start(service.id)
        let deadline = Date.now.addingTimeInterval(15)
        while model.service(service.id)?.status.isLive != false || model.service(service.id)?.runNumber == nil {
            guard Date.now < deadline else { break }
            try await Task.sleep(for: .milliseconds(50))
            model.refresh()
        }
        let state = try #require(model.service(service.id))
        let lines = model.logLines(service.id, from: state.logFirst, max: 100)
        #expect(lines.map(\.text).contains("one"))
        #expect(lines.map(\.text).contains("two"))
        #expect(lines.first?.stream == .system)

        // Bunode's own line ("Run #1: printf 'one\ntwo\n'") matches too.
        model.setLogFilter(service.id, "TWO")
        let searchDeadline = Date.now.addingTimeInterval(5)
        while model.service(service.id)?.logMatchCount != 2, Date.now < searchDeadline {
            try await Task.sleep(for: .milliseconds(20))
            model.refresh()
        }
        let match = try #require(model.service(service.id))
        let matches = model.logMatchingLines(service.id, from: match.logMatchFirst, max: 10).map(\.text)
        #expect(matches.count == 2)
        #expect(matches.last == "two")

        model.clearLogs(service.id)
        #expect(model.service(service.id)?.logCount == 0)
    }
}

struct ExportNameTests {
    @Test func namesExportsAfterTheRun() {
        #expect(AppModel.exportFileName(project: "api", service: "dev", run: 14) == "api-dev-run14.log")
        #expect(AppModel.exportFileName(project: "acme", service: "web / dev", run: 2) == "acme-web---dev-run2.log")
        #expect(AppModel.exportFileName(project: "api", service: "dev", run: nil) == "api-dev.log")
    }
}

@MainActor
struct ExportTests {
    let data = TemporaryDirectory()
    let code = TemporaryDirectory()

    @Test func exportsTheFullLog() async throws {
        let model = AppModel(dataDirectory: data.url)
        _ = await model.addProject(at: code.folder("app"))
        let project = try #require(model.snapshot.projects.first)
        let service = try await model.addService(to: project.id, name: "echo", command: "printf 'hello\\n'", folder: nil).get()
        model.start(service.id)
        let deadline = Date.now.addingTimeInterval(15)
        while model.service(service.id)?.runNumber == nil || model.service(service.id)?.status.isLive == true {
            guard Date.now < deadline else { break }
            try await Task.sleep(for: .milliseconds(50))
            model.refresh()
        }
        let out = code.url.appending(path: "export.log")
        #expect(await model.exportLog(service.id, to: out) == nil)
        let text = try String(contentsOf: out, encoding: .utf8)
        #expect(text.contains(" out hello"))
        #expect(text.contains(" bunode Run #1: printf"))
    }
}
