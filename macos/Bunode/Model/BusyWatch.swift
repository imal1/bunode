import Foundation

/// Notices Services stuck in Starting or Stopping. If the Supervisor sends
/// nothing for longer than Force stop + 5 s, the row says Not responding
/// and offers Stop again, which then force-kills (design-handoff §5.1).
struct BusyWatch {
    /// When each busy Service was first seen busy.
    private(set) var since: [UInt64: Date] = [:]

    /// Records which Services are busy now; any that is no longer busy is
    /// forgotten.
    mutating func update(busy: Set<UInt64>, now: Date) {
        since = since.filter { busy.contains($0.key) }
        for id in busy where since[id] == nil {
            since[id] = now
        }
    }

    func overdue(now: Date, timeout: TimeInterval) -> Set<UInt64> {
        Set(since.filter { now.timeIntervalSince($0.value) > timeout }.keys)
    }

    var isEmpty: Bool { since.isEmpty }
}
