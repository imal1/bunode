import AppKit

/// Entry point. Bunode is an agent app (`LSUIElement`): it lives in the menu
/// bar (ADR-0004), and shows a Dock icon and the app menu bar only while the
/// Settings window is open (ADR-0009).
@main
struct BunodeMain {
    @MainActor
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        // `NSApplication.delegate` is weak, so keep ours alive while the app runs.
        withExtendedLifetime(delegate) {
            app.run()
        }
    }
}
