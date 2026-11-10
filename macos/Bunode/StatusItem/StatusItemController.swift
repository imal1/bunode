import AppKit
import SwiftUI

/// Owns the menu bar status item and the popover it opens
/// (design-handoff §2.1, §2.2).
@MainActor
final class StatusItemController: NSObject, NSPopoverDelegate {
    private let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private let popover: NSPopover = NSPopover()
    private let router = PopoverRouter()
    private let toaster = Toaster()
    private let dot = StatusDotView()
    private let model: AppModel
    /// When the popover last closed; see `togglePopover(_:)`.
    private var lastClosed = Date.distantPast
    /// NSPopover leaves this gap where the hidden arrow was. Raising its
    /// anchor by as much puts the top edge against the menu bar, and holds
    /// when the popover repositions itself after a height change.
    private var anchorRaise: CGFloat = 0

    /// `openSettings(pane, addProject)` opens the Settings window, optionally
    /// on a pane and with the Add Project folder picker.
    /// `quit` is called once every Run has stopped.
    init(model: AppModel, openSettings: @escaping (SettingsPane?, Bool) -> Void, quit: @escaping () -> Void) {
        self.model = model
        super.init()

        let content = NSHostingController(
            rootView: PopoverView(
                model: model,
                router: router,
                toaster: toaster,
                openSettings: openSettings,
                quit: quit,
                close: { [weak popover] in popover?.performClose(nil) }
            )
        )
        // The popover takes its size from the SwiftUI content: 340 pt wide,
        // as tall as it needs.
        content.sizingOptions = .preferredContentSize
        popover.contentViewController = content
        popover.behavior = .transient
        popover.delegate = self
        // No arrow. NSPopover has no public switch for it; checking first
        // means a macOS without the key keeps the arrow instead of crashing.
        if popover.responds(to: NSSelectorFromString("setShouldHideAnchor:")) {
            popover.setValue(true, forKey: "shouldHideAnchor")
            anchorRaise = 9
        }

        if let button = statusItem.button {
            button.image = StatusIcon.hexagon()
            button.imagePosition = .imageLeading
            button.font = .systemFont(ofSize: 12, weight: .semibold)
            button.target = self
            button.action = #selector(togglePopover(_:))
            button.addSubview(dot)
            button.postsFrameChangedNotifications = true
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(buttonFrameDidChange(_:)),
                name: NSView.frameDidChangeNotification,
                object: button
            )
        }
        update(model.statusSummary)
        observe(model)
    }

    /// Keeps the icon in step with the model.
    private func observe(_ model: AppModel) {
        withObservationTracking {
            update(model.statusSummary)
        } onChange: { [weak self, weak model] in
            Task { @MainActor in
                guard let self, let model else { return }
                self.observe(model)
            }
        }
    }

    func update(_ summary: StatusSummary) {
        guard let button = statusItem.button else { return }
        button.title = summary.countText ?? ""
        button.setAccessibilityLabel(summary.accessibilityLabel)
        dot.color = summary.aggregate.color
        layoutDot()
    }

    func closePopover() {
        if popover.isShown {
            popover.performClose(nil)
        }
    }

    // MARK: - Popover

    @objc private func togglePopover(_ sender: Any?) {
        if popover.isShown {
            popover.performClose(sender)
        } else if Date().timeIntervalSince(lastClosed) > 0.25 {
            // A click on the icon while the popover is open first closes the
            // transient popover (a mouse down outside it) and only then sends
            // this action. Without this check the popover would reopen at once.
            showPopover()
        }
    }

    private func showPopover() {
        guard let button = statusItem.button else { return }
        router.reset()
        model.checkLaunchFiles()
        // Make the popover key, so its keyboard shortcuts (⌘, ⌘Q) work.
        NSApp.activate()
        let anchor = button.bounds.offsetBy(dx: 0, dy: button.isFlipped ? -anchorRaise : anchorRaise)
        popover.show(relativeTo: anchor, of: button, preferredEdge: .minY)
        popover.contentViewController?.view.window?.makeKey()
        button.highlight(true)
    }

    func popoverDidClose(_ notification: Notification) {
        lastClosed = Date()
        statusItem.button?.highlight(false)
    }

    // MARK: - Dot

    @objc private func buttonFrameDidChange(_ notification: Notification) {
        layoutDot()
    }

    /// Centers the dot on the hexagon, wherever the button lays out its image.
    private func layoutDot() {
        guard let button = statusItem.button, let cell = button.cell else { return }
        button.layoutSubtreeIfNeeded()
        let image = cell.imageRect(forBounds: button.bounds)
        let diameter = StatusIcon.dotDiameter
        dot.frame = NSRect(
            x: image.midX - diameter / 2,
            y: image.midY - diameter / 2,
            width: diameter,
            height: diameter
        )
    }
}

extension StatusSummary.Aggregate {
    var color: NSColor {
        switch self {
        case .idle: .systemGray
        case .running: .systemGreen
        case .error: .systemRed
        }
    }
}
