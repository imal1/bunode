import Foundation

/// The lines of one Service's log that the Logs view shows, kept in step
/// with the core: each update fetches only the new lines and drops the
/// ones the core no longer keeps.
@MainActor
final class LogMirror {
    enum Change: Equatable {
        case none
        /// Start over (new Run, Clear, or too much changed at once).
        case reload
        /// `removed` lines left the top, `appended` arrived at the bottom.
        case update(removed: Int, appended: Int)
    }

    private(set) var lines: [LogLine] = []
    private(set) var first: UInt64 = 0
    /// Anything bigger is a reload: inserting thousands of rows one update
    /// at a time is slower than reloading.
    private let largeUpdate = 2_000

    var count: Int { lines.count }

    /// Fetches up to `max` lines starting at `from`.
    typealias Fetch = (_ from: UInt64, _ max: UInt32) -> [LogLine]

    func sync(first newFirst: UInt64, count newCount: UInt64, fetch: Fetch) -> Change {
        let end = first + UInt64(lines.count)
        let newEnd = newFirst + newCount
        if newFirst == first, newEnd == end {
            return .none
        }
        let continues = !lines.isEmpty && newFirst >= first && newFirst <= end && newEnd >= end
        if !continues {
            first = newFirst
            lines = fetchAll(from: newFirst, count: newCount, fetch: fetch)
            return .reload
        }
        let removed = Int(newFirst - first)
        lines.removeFirst(removed)
        first = newFirst
        let appended = fetchAll(from: end, count: newEnd - end, fetch: fetch)
        lines.append(contentsOf: appended)
        if removed + appended.count > largeUpdate {
            return .reload
        }
        return .update(removed: removed, appended: appended.count)
    }

    func line(at row: Int) -> LogLine? {
        lines.indices.contains(row) ? lines[row] : nil
    }

    private func fetchAll(from: UInt64, count: UInt64, fetch: Fetch) -> [LogLine] {
        var result: [LogLine] = []
        result.reserveCapacity(Int(count))
        var next = from
        while UInt64(result.count) < count {
            let chunk = fetch(next, UInt32(min(5_000, count - UInt64(result.count))))
            guard let last = chunk.last else { break }
            result.append(contentsOf: chunk)
            next = last.seq + 1
        }
        return result
    }
}
