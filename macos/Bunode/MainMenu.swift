import AppKit

/// The app's main menu. An agent app never shows a menu bar, but AppKit still
/// routes key equivalents through this menu. That is what gives the Settings
/// window ⌘, ⌘Q and ⌘W, and text fields the standard editing shortcuts.
@MainActor
enum MainMenu {
    static func make() -> NSMenu {
        let menu = NSMenu()
        menu.addItem(submenu("Bunode", [
            item(String(localized: "Settings…"), #selector(AppDelegate.openSettings(_:)), ","),
            .separator(),
            item(String(localized: "Quit Bunode"), #selector(NSApplication.terminate(_:)), "q"),
        ]))
        menu.addItem(submenu(String(localized: "Edit"), [
            item(String(localized: "Undo"), Selector(("undo:")), "z"),
            item(String(localized: "Redo"), Selector(("redo:")), "Z"),
            .separator(),
            item(String(localized: "Cut"), #selector(NSText.cut(_:)), "x"),
            item(String(localized: "Copy"), #selector(NSText.copy(_:)), "c"),
            item(String(localized: "Paste"), #selector(NSText.paste(_:)), "v"),
            item(String(localized: "Select All"), #selector(NSText.selectAll(_:)), "a"),
        ]))
        menu.addItem(submenu(String(localized: "Window"), [
            item(String(localized: "Close"), #selector(NSWindow.performClose(_:)), "w"),
            item(String(localized: "Minimize"), #selector(NSWindow.performMiniaturize(_:)), "m"),
        ]))
        return menu
    }

    /// A menu item sent down the responder chain. An uppercase key adds ⇧.
    private static func item(_ title: String, _ action: Selector, _ key: String) -> NSMenuItem {
        NSMenuItem(title: title, action: action, keyEquivalent: key)
    }

    private static func submenu(_ title: String, _ items: [NSMenuItem]) -> NSMenuItem {
        let submenu = NSMenu(title: title)
        for item in items {
            submenu.addItem(item)
        }
        let item = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        item.submenu = submenu
        return item
    }
}
