import AppKit
import SwiftUI

/// The popover's view stack (design-handoff §2.2). One view shows at a
/// time; opening the popover always starts at Home.
@MainActor
@Observable
final class PopoverRouter {
    enum Screen: Hashable {
        case home
        case addService(projectID: UInt64)
    }

    private(set) var stack: [Screen] = [.home]
    /// Whether the last change went deeper, for the push transition.
    private(set) var isForward = true

    var screen: Screen { stack.last ?? .home }

    func push(_ screen: Screen) {
        change(forward: true) { stack.append(screen) }
    }

    func back() {
        guard stack.count > 1 else { return }
        change(forward: false) { stack.removeLast() }
    }

    /// Back to Home, e.g. after adding a Service.
    func popToHome() {
        guard stack.count > 1 else { return }
        change(forward: false) { stack = [.home] }
    }

    /// Back to Home without animation, when the popover opens.
    func reset() {
        stack = [.home]
    }

    private func change(forward: Bool, _ update: () -> Void) {
        isForward = forward
        if NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            update()
        } else {
            withAnimation(.easeOut(duration: 0.15), update)
        }
    }
}
