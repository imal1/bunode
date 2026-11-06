import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var model: AppModel?
    private var settingsWindow: SettingsWindowController?
    private var statusItem: StatusItemController?

    /// Unit tests run inside the app; they build their own models.
    private static var isRunningTests: Bool {
        ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard !Self.isRunningTests else { return }
        NSApp.mainMenu = MainMenu.make()
        let model = AppModel(dataDirectory: AppModel.defaultDataDirectory)
        self.model = model
        settingsWindow = SettingsWindowController(model: model)
        statusItem = StatusItemController(
            model: model,
            openSettings: { [weak self] pane, addProject in
                self?.showSettings(pane: pane, addProject: addProject)
            }
        )
    }

    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
        true
    }

    /// Opens the Settings window, or brings it to the front (⌘,).
    @objc func openSettings(_ sender: Any?) {
        showSettings(pane: nil, addProject: false)
    }

    private func showSettings(pane: SettingsPane?, addProject: Bool) {
        statusItem?.closePopover()
        settingsWindow?.show(pane: pane, addProject: addProject)
    }
}
