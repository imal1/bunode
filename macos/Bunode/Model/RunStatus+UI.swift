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

    /// The note on a Home row; only Crashed and Failed have one.
    var rowNote: String? {
        switch self {
        case .crashed: String(localized: "Crashed")
        case .failed: String(localized: "Failed")
        default: nil
        }
    }
}
