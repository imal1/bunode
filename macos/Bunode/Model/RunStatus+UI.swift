import SwiftUI

extension RunStatus {
    /// A Run exists and may still own processes.
    var isLive: Bool {
        switch self {
        case .starting, .running, .stopping, .restarting: true
        case .stopped, .crashed, .failed: false
        }
    }

    /// Waiting for the Supervisor: the row shows a spinner instead of a button.
    var isBusy: Bool {
        switch self {
        case .starting, .stopping, .restarting: true
        default: false
        }
    }

    var dotStyle: DotStyle {
        switch self {
        case .stopped: .stopped
        case .running: .running
        case .starting, .stopping, .restarting: .busy
        case .crashed, .failed: .failed
        }
    }

    /// The word VoiceOver and the Service view use.
    var label: String {
        switch self {
        case .stopped: String(localized: "Stopped")
        case .starting: String(localized: "Starting…")
        case .running: String(localized: "Running")
        case .stopping: String(localized: "Stopping…")
        case .restarting: String(localized: "Restarting…")
        case .crashed: String(localized: "Crashed")
        case .failed: String(localized: "Failed")
        }
    }
}

extension ServiceState {
    /// The script name and category of a preset command (design-handoff §2.2).
    private static func preset(_ command: String) -> (script: String, category: String)? {
        command.wholeMatch(of: /(?:bun|npm|pnpm|yarn)(?:\s+run)?\s+((dev|start|serve|preview|build|test|lint)(?:[:-]\S+)?)/)
            .map { (String($0.1), String($0.2)) }
    }

    /// What a row and the Service view title call a command: the script name
    /// of a preset, otherwise the whole command.
    static func shortCommand(_ command: String) -> String {
        preset(command)?.script ?? command
    }

    /// A build, test or lint script ends by itself, unless it watches.
    /// Everything else is treated as a long-running server.
    var isShortLived: Bool {
        guard let preset = Self.preset(command), ["build", "test", "lint"].contains(preset.category) else {
            return false
        }
        return !preset.script.hasSuffix(":watch") && !preset.script.hasSuffix("-watch")
    }

    /// `status.dotStyle`, except that a short-lived Service in progress isn't
    /// green, and shows when its last Run finished by itself.
    func dotStyle(status: RunStatus) -> DotStyle {
        guard isShortLived else { return status.dotStyle }
        switch status {
        case .stopped: return endedBy == .exited ? .done : .stoppedShortLived
        case .running: return .busy
        default: return status.dotStyle
        }
    }

    /// `status.label`, in the short-lived Service's words (design-handoff §5.1).
    func statusLabel(status: RunStatus) -> String {
        guard isShortLived else { return status.label }
        switch status {
        case .stopped where endedBy == .exited: return String(localized: "Done")
        case .running: return String(localized: "Running…")
        case .crashed: return String(localized: "Failed")
        default: return status.label
        }
    }

    /// What the button that begins a Run says.
    var startTitle: String {
        isShortLived ? String(localized: "Run") : String(localized: "Start")
    }
}
