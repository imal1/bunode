import AppKit
import SwiftUI

/// The Settings window (⌘,): a standard window with a fixed 700×540 pt
/// content size that cannot be resized (design-handoff §2.3).
@MainActor
final class SettingsWindowController {
    static let contentSize = NSSize(width: 700, height: 540)

    private var window: NSWindow?

    /// Opens the window, or brings it to the front if it is already open.
    func show() {
        let window = self.window ?? makeWindow()
        self.window = window
        // An agent app is not active by default; without this the window
        // would open behind the frontmost app.
        NSApp.activate()
        window.makeKeyAndOrderFront(nil)
    }

    private func makeWindow() -> NSWindow {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: Self.contentSize),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: true
        )
        window.title = String(localized: "Bunode Settings")
        window.contentViewController = NSHostingController(rootView: SettingsView())
        window.setContentSize(Self.contentSize)
        window.isReleasedWhenClosed = false
        window.center()
        return window
    }
}
