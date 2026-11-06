import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let core = Core()
    private let settingsWindow = SettingsWindowController()
    private var statusItem: StatusItemController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = MainMenu.make()
        statusItem = StatusItemController(
            coreVersion: core.version,
            openSettings: { [weak self] in self?.openSettings(nil) }
        )
    }

    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
        true
    }

    /// Opens the Settings window, or brings it to the front (⌘,).
    @objc func openSettings(_ sender: Any?) {
        statusItem?.closePopover()
        settingsWindow.show()
    }
}
