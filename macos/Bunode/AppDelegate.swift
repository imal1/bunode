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
    /// Bunode opens again after it quits (Settings › Language, ADR-0006).
    private var relaunching = false

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
            let (title, text, confirm) = relaunching
                ? (String(localized: "Restart Bunode?"),
                   String(localized: "\(count) Services are running. Bunode stops every Run before it restarts."),
                   String(localized: "Stop \(count) Services and Restart"))
                : (String(localized: "Quit Bunode?"),
                   String(localized: "\(count) Services are running. Bunode stops every Run before it quits, so it never leaves processes behind."),
                   String(localized: "Stop \(count) Services and Quit"))
            let alert = NSAlert.withAppIcon()
            alert.messageText = title
            alert.informativeText = text
            alert.addButton(withTitle: confirm)
            alert.addButton(withTitle: String(localized: "Cancel"))
            NSApp.activate()
            guard alert.runModal() == .alertFirstButtonReturn else {
                relaunching = false
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

    /// Settings › Language's Restart Now: quits through the usual
    /// confirmation, then opens again.
    @objc func restart(_ sender: Any?) {
        relaunching = true
        NSApp.terminate(sender)
    }

    func applicationWillTerminate(_ notification: Notification) {
        // A shutdown or logout that came after Restart Now wins.
        guard relaunching, !isPoweringOff else { return }
        // Not a Run, so no lifeline: it outlives Bunode, waits until this
        // process is gone (`open` would only bring a quitting Bunode to the
        // front), and opens this copy of the app by path; a bundle id could
        // pick another copy.
        let reopen = Process()
        reopen.executableURL = URL(fileURLWithPath: "/bin/sh")
        reopen.arguments = [
            "-c", "sleep 1; while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; open \"$0\"",
            Bundle.main.bundlePath, String(ProcessInfo.processInfo.processIdentifier),
        ]
        try? reopen.run()
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

    // MARK: App and Help menus

    /// The standard About panel: icon, name, `Version 0.1.0 (42)` from the
    /// bundle (scripts/version.sh), the copyright and a link to the repository.
    @objc func showAbout(_ sender: Any?) {
        let credits = NSMutableAttributedString(
            string: String(localized: "A menu bar home for your Node.js and Bun dev servers."),
            attributes: [.font: NSFont.systemFont(ofSize: 11), .foregroundColor: NSColor.secondaryLabelColor]
        )
        credits.append(NSAttributedString(string: "\n"))
        credits.append(NSAttributedString(
            string: "github.com/imal1/bunode",
            attributes: [.font: NSFont.systemFont(ofSize: 11), .link: MainMenu.repository]
        ))
        let centered = NSMutableParagraphStyle()
        centered.alignment = .center
        credits.addAttribute(.paragraphStyle, value: centered, range: NSRange(location: 0, length: credits.length))
        NSApp.activate()
        NSApp.orderFrontStandardAboutPanel(options: [.credits: credits, .applicationIcon: AppIcon.image])
    }

    @objc func openRepository(_ sender: Any?) {
        NSWorkspace.shared.open(MainMenu.repository)
    }

    /// The GitHub release of this version; its notes come from CHANGELOG.md.
    @objc func openReleaseNotes(_ sender: Any?) {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        NSWorkspace.shared.open(MainMenu.repository.appending(path: "releases/tag/v\(version)"))
    }

    @objc func reportIssue(_ sender: Any?) {
        NSWorkspace.shared.open(MainMenu.repository.appending(path: "issues/new"))
    }
}
