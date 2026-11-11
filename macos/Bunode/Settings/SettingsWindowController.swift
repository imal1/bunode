import AppKit
import SwiftUI

/// The Settings window (⌘,): a standard window with a fixed 700×540 pt
/// content size that cannot be resized (design-handoff §2.3).
@MainActor
final class SettingsWindowController {
    static let contentSize = NSSize(width: 700, height: 540)

    private let model: AppModel
    private let navigation = SettingsNavigation()
    private var window: NSWindow?

    init(model: AppModel) {
        self.model = model
    }

    /// Opens the window, or brings it to the front if it is already open.
    func show(pane: SettingsPane? = nil, addProject: Bool = false) {
        if let pane {
            navigation.pane = pane
        }
        let window = self.window ?? makeWindow()
        self.window = window
        // While Settings is open Bunode is a regular app: the app menu bar
        // (About, ⌘Q…) and a Dock icon, so ⌘Tab finds the window (ADR-0009).
        _ = NSApp.setActivationPolicy(.regular)
        // An agent app is not active by default; without this the window
        // would open behind the frontmost app.
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
        if addProject {
            navigation.pane = .projects
            navigation.pendingAddProject = true
        }
    }

    private func makeWindow() -> NSWindow {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: Self.contentSize),
            styleMask: [.titled, .closable, .miniaturizable, .fullSizeContentView],
            backing: .buffered,
            defer: true
        )
        window.title = String(localized: "Bunode Settings")
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.contentViewController = NSHostingController(
            rootView: SettingsView(model: model, navigation: navigation)
        )
        window.setContentSize(Self.contentSize)
        window.isReleasedWhenClosed = false
        window.center()
        NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: window,
            queue: .main
        ) { _ in
            // Back to a menu bar-only agent app.
            MainActor.assumeIsolated {
                _ = NSApp.setActivationPolicy(.accessory)
            }
        }
        return window
    }
}
