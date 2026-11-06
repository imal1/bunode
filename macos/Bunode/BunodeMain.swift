import AppKit

/// Entry point. Bunode is an agent app (`LSUIElement`): no Dock icon and no
/// app menu bar. It lives in the menu bar (ADR-0004).
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
