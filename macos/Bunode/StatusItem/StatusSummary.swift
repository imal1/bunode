import Foundation

/// What the menu bar status item shows (design-handoff §2.1). Derived from
/// the Supervisor's state; the status item never computes it itself.
struct StatusSummary: Equatable {
    /// Long-running Services whose Run is `running` (not starting or
    /// stopping). A build in progress isn't something left running.
    var running = 0
    /// Services whose last Run crashed or failed.
    var crashed = 0
    /// General › Show running count.
    var showsCount = true

    /// The color of the dot in the middle of the icon.
    enum Aggregate: Equatable {
        case idle, running, error
    }

    var aggregate: Aggregate {
        if crashed > 0 { return .error }
        if running > 0 { return .running }
        return .idle
    }

    /// The number next to the icon, or nil when it is hidden. It is hidden
    /// at zero, and when the user turned it off.
    var countText: String? {
        guard showsCount, running > 0 else { return nil }
        return String(running)
    }

    /// VoiceOver label: "Bunode, 2 running" or "Bunode, 2 running, 1 crashed".
    var accessibilityLabel: String {
        var parts = ["Bunode", String(localized: "\(running) running")]
        if crashed > 0 {
            parts.append(String(localized: "\(crashed) crashed"))
        }
        return parts.joined(separator: ", ")
    }
}

extension StatusSummary {
    init(services: [ServiceState], status: (ServiceState) -> RunStatus, showsCount: Bool) {
        running = services.count(where: { status($0) == .running && !$0.isShortLived })
        crashed = services.count(where: { status($0) == .crashed || status($0) == .failed })
        self.showsCount = showsCount
    }
}
