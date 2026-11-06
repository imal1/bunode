import Foundation

/// Connects a Logs view to its table: syncs the mirror from the core and
/// hands each change to the table right away, so no change is lost
/// between two SwiftUI updates.
@MainActor
final class LogFeed {
    let mirror = LogMirror()
    weak var controller: LogTableController? {
        didSet { controller?.apply(.reload) }
    }

    func sync(first: UInt64, count: UInt64, fetch: LogMirror.Fetch) {
        let change = mirror.sync(first: first, count: count, fetch: fetch)
        controller?.apply(change)
    }
}
