import Foundation

/// Small conveniences over the generated UniFFI bindings (`bunode_ffi.swift`).
enum Core {
    /// The version of the Rust core linked into the app, such as "0.1.0".
    static var version: String {
        coreVersion()
    }
}

/// Receives the core's change notifications on its background thread and
/// hands them to the main actor as an async sequence.
final class CoreListenerBridge: CoreListener {
    let changes: AsyncStream<Void>
    private let continuation: AsyncStream<Void>.Continuation

    init() {
        (changes, continuation) = AsyncStream.makeStream(of: Void.self, bufferingPolicy: .bufferingNewest(1))
    }

    func stateChanged() {
        continuation.yield()
    }
}

extension PackageManager {
    /// "npm", "pnpm", "yarn", "Bun".
    var displayName: String {
        switch self {
        case .npm: "npm"
        case .pnpm: "pnpm"
        case .yarn: "yarn"
        case .bun: "Bun"
        }
    }
}

extension Error {
    /// The message to show the user. Core errors carry one already.
    var userMessage: String {
        if let error = self as? CoreError, case .Failed(let message) = error {
            return message
        }
        return localizedDescription
    }
}

extension String {
    /// `/Users/me/code/api` → `~/code/api`.
    var abbreviatingWithTilde: String {
        (self as NSString).abbreviatingWithTildeInPath
    }
}
