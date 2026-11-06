import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var model: AppModel?
    private var settingsWindow: SettingsWindowController?
    private var statusItem: StatusItemController?
    /// Every Run has been stopped; quitting needs no more questions.
    private var readyToQuit = false
    /// The Mac is shutting down or the user is logging out.
    private var isPoweringOff = false

    /// Unit tests run inside the app; they build their own models.
    private static var isRunningTests: Bool {
        ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard !Self.isRunningTests else { return }
        NSApp.mainMenu = MainMenu.make()
        let model = AppModel(
            dataDirectory: AppModel.defaultDataDirectory,
            diagnostics: AppModel.defaultDiagnosticsDirectory
        )
        self.model = model
        settingsWindow = SettingsWindowController(model: model)
        statusItem = StatusItemController(
            model: model,
            openSettings: { [weak self] pane, addProject in
                self?.showSettings(pane: pane, addProject: addProject)
            },
            quit: { [weak self] in self?.quitAfterStopping() }
        )
        NSWorkspace.shared.notificationCenter.addObserver(
            self,
            selector: #selector(willPowerOff(_:)),
            name: NSWorkspace.willPowerOffNotification,
            object: nil
        )
    }

    @objc private func willPowerOff(_ notification: Notification) {
        isPoweringOff = true
    }

    /// The popover's Quit, after its own confirmation: every Run is
    /// stopped already.
    private func quitAfterStopping() {
        readyToQuit = true
        NSApp.terminate(nil)
    }

    /// ⌘Q from anywhere but the popover, logout and shutdown
    /// (design-handoff §5.6). Bunode never quits leaving Runs behind.
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let model, !readyToQuit, model.liveCount > 0 else {
            return .terminateNow
        }
        if !isPoweringOff {
            let count = model.liveCount
            let alert = NSAlert()
            alert.messageText = String(localized: "Quit Bunode?")
            alert.informativeText = String(localized: "\(count) Services are running. Bunode stops every Run before it quits, so it never leaves processes behind.")
            alert.addButton(withTitle: String(localized: "Stop \(count) Services and Quit"))
            alert.addButton(withTitle: String(localized: "Cancel"))
            NSApp.activate()
            guard alert.runModal() == .alertFirstButtonReturn else {
                return .terminateCancel
            }
        }
        // Shutting down or logging out: no question, just stop every Run.
        Task {
            await model.stopAllAndWait()
            readyToQuit = true
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
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
