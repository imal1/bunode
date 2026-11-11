import AppKit

/// The app's main menu. It is on screen only while the Settings window is
/// open (ADR-0009); the rest of the time AppKit still routes key equivalents
/// through it, which gives text fields the standard editing shortcuts.
@MainActor
enum MainMenu {
    static let repository = URL(string: "https://github.com/imal1/bunode")!

    static func make() -> NSMenu {
        let menu = NSMenu()
        // AppKit shows the app's name as the first menu's title.
        let services = NSMenu(title: String(localized: "Services"))
        menu.addItem(submenu("Bunode", [
            item(String(localized: "About Bunode"), #selector(AppDelegate.showAbout(_:))),
            .separator(),
            item(String(localized: "Settings…"), #selector(AppDelegate.openSettings(_:)), ","),
            .separator(),
            submenuItem(services),
            .separator(),
            item(String(localized: "Hide Bunode"), #selector(NSApplication.hide(_:)), "h"),
            item(String(localized: "Hide Others"), #selector(NSApplication.hideOtherApplications(_:)), "h", [.command, .option]),
            item(String(localized: "Show All"), #selector(NSApplication.unhideAllApplications(_:))),
            .separator(),
            item(String(localized: "Quit Bunode"), #selector(NSApplication.terminate(_:)), "q"),
        ]))
        NSApp.servicesMenu = services
        menu.addItem(submenu(String(localized: "Edit"), [
            item(String(localized: "Undo"), Selector(("undo:")), "z"),
            item(String(localized: "Redo"), Selector(("redo:")), "Z"),
            .separator(),
            item(String(localized: "Cut"), #selector(NSText.cut(_:)), "x"),
            item(String(localized: "Copy"), #selector(NSText.copy(_:)), "c"),
            item(String(localized: "Paste"), #selector(NSText.paste(_:)), "v"),
            item(String(localized: "Select All"), #selector(NSText.selectAll(_:)), "a"),
        ]))
        let window = submenu(String(localized: "Window"), [
            item(String(localized: "Close"), #selector(NSWindow.performClose(_:)), "w"),
            item(String(localized: "Minimize"), #selector(NSWindow.performMiniaturize(_:)), "m"),
        ])
        menu.addItem(window)
        NSApp.windowsMenu = window.submenu
        let help = submenu(String(localized: "Help"), [
            item(String(localized: "Bunode on GitHub"), #selector(AppDelegate.openRepository(_:))),
            item(String(localized: "Release Notes"), #selector(AppDelegate.openReleaseNotes(_:))),
            item(String(localized: "Report an Issue…"), #selector(AppDelegate.reportIssue(_:))),
        ])
        menu.addItem(help)
        NSApp.helpMenu = help.submenu
        return menu
    }

    /// A menu item sent down the responder chain. An uppercase key adds ⇧.
    private static func item(
        _ title: String,
        _ action: Selector,
        _ key: String = "",
        _ modifiers: NSEvent.ModifierFlags = .command
    ) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.keyEquivalentModifierMask = modifiers
        return item
    }

    private static func submenu(_ title: String, _ items: [NSMenuItem]) -> NSMenuItem {
        let submenu = NSMenu(title: title)
        for item in items {
            submenu.addItem(item)
        }
        return submenuItem(submenu)
    }

    private static func submenuItem(_ submenu: NSMenu) -> NSMenuItem {
        let item = NSMenuItem(title: submenu.title, action: nil, keyEquivalent: "")
        item.submenu = submenu
        return item
    }
}
