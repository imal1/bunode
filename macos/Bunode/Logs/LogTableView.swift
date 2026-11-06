import AppKit
import SwiftUI

/// The log area (design-handoff §2.2 Logs): a virtualized AppKit table of
/// time + text rows on a dark background. It follows new lines while
/// scrolled to the bottom and stays put once the user scrolls up.
struct LogTableView: NSViewRepresentable {
    let feed: LogFeed
    let accessibilityName: String

    func makeCoordinator() -> LogTableController {
        LogTableController(mirror: feed.mirror)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let controller = context.coordinator
        controller.table.setAccessibilityLabel(accessibilityName)
        feed.controller = controller
        return controller.scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        context.coordinator.table.setAccessibilityLabel(accessibilityName)
    }
}

@MainActor
final class LogTableController: NSObject, NSTableViewDataSource, NSTableViewDelegate {
    let scrollView = NSScrollView()
    let table = LogTable()

    private let mirror: LogMirror
    private let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("line"))

    /// Row layout (design-handoff §2.2): 10 pt padding, 58 pt time column,
    /// 8 pt gap, the text.
    static let padding: CGFloat = 10
    static let timeWidth: CGFloat = 58
    static let gap: CGFloat = 8

    init(mirror: LogMirror) {
        self.mirror = mirror
        super.init()

        column.resizingMask = .autoresizingMask
        table.addTableColumn(column)
        table.headerView = nil
        table.backgroundColor = LogPalette.background
        table.gridStyleMask = []
        table.intercellSpacing = .zero
        table.selectionHighlightStyle = .regular
        table.allowsMultipleSelection = true
        table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        table.style = .plain
        table.dataSource = self
        table.delegate = self
        table.mirror = mirror
        table.setAccessibilityRole(.list)

        scrollView.documentView = table
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = true
        scrollView.backgroundColor = LogPalette.background
        scrollView.automaticallyAdjustsContentInsets = false
        scrollView.contentInsets = NSEdgeInsets(top: 8, left: 0, bottom: 8, right: 0)
        scrollView.wantsLayer = true
        scrollView.layer?.cornerRadius = 9
        scrollView.layer?.masksToBounds = true

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(widthDidChange),
            name: NSView.frameDidChangeNotification,
            object: table
        )
        table.postsFrameChangedNotifications = true
    }

    /// Characters that fit on a line of the text column.
    private var columns: Int {
        let width = table.bounds.width - Self.padding * 2 - Self.timeWidth - Self.gap
        return max(1, Int(width / LogPalette.characterWidth))
    }

    private var lastColumns = 0

    @objc private func widthDidChange() {
        guard columns != lastColumns else { return }
        lastColumns = columns
        table.noteHeightOfRows(withIndexesChanged: IndexSet(integersIn: 0..<mirror.count))
    }

    func apply(_ change: LogMirror.Change) {
        let followsTail = isScrolledToBottom
        switch change {
        case .none:
            return
        case .reload:
            table.reloadData()
        case .update(let removed, let appended):
            table.beginUpdates()
            if removed > 0 {
                table.removeRows(at: IndexSet(integersIn: 0..<removed), withAnimation: [])
            }
            if appended > 0 {
                let start = mirror.count - appended
                table.insertRows(at: IndexSet(integersIn: start..<mirror.count), withAnimation: [])
            }
            table.endUpdates()
        }
        if followsTail, mirror.count > 0 {
            table.scrollRowToVisible(mirror.count - 1)
        }
    }

    /// Within a few points of the end, or there is nothing to scroll.
    private var isScrolledToBottom: Bool {
        let visible = scrollView.contentView.bounds
        let insets = scrollView.contentInsets
        return visible.maxY + insets.bottom >= table.bounds.height - 4
    }

    // MARK: - Data source and delegate

    func numberOfRows(in tableView: NSTableView) -> Int {
        mirror.count
    }

    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
        guard let line = mirror.line(at: row) else { return LogPalette.lineHeight }
        let lines = LogPalette.wrappedLineCount(of: line.text, columns: columns)
        return CGFloat(lines) * LogPalette.lineHeight
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let identifier = NSUserInterfaceItemIdentifier("LogRow")
        let cell = (tableView.makeView(withIdentifier: identifier, owner: nil) as? LogRowView) ?? {
            let cell = LogRowView()
            cell.identifier = identifier
            return cell
        }()
        if let line = mirror.line(at: row) {
            cell.show(line)
        }
        return cell
    }
}

/// The table, with ⌘C copying the selected lines.
final class LogTable: NSTableView {
    weak var mirror: LogMirror?

    @objc func copy(_ sender: Any?) {
        guard let mirror else { return }
        let lines = selectedRowIndexes.compactMap { mirror.line(at: $0) }
        guard !lines.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(LogPalette.plainText(of: lines), forType: .string)
    }
}

/// One log row: the time, then the wrapped, colored text.
final class LogRowView: NSTableCellView {
    private let time = NSTextField(labelWithString: "")
    private let text = NSTextField(wrappingLabelWithString: "")

    override init(frame: NSRect) {
        super.init(frame: frame)
        time.font = LogPalette.font
        time.textColor = LogPalette.time
        time.lineBreakMode = .byClipping
        text.isSelectable = false
        text.lineBreakMode = .byCharWrapping
        text.maximumNumberOfLines = 0
        text.cell?.wraps = true
        text.cell?.truncatesLastVisibleLine = false
        for field in [time, text] {
            field.drawsBackground = false
            field.isBordered = false
            addSubview(field)
        }
        setAccessibilityElement(true)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not used")
    }

    func show(_ line: LogLine) {
        let stamp = LogPalette.time(of: line)
        time.attributedStringValue = LogPalette.attributedTime(stamp)
        text.attributedStringValue = LogPalette.attributedText(of: line)
        setAccessibilityLabel("\(stamp) \(line.text)")
        needsLayout = true
    }

    override func layout() {
        super.layout()
        let padding = LogTableController.padding
        let timeWidth = LogTableController.timeWidth
        let textX = padding + timeWidth + LogTableController.gap
        // NSTextField adds a 2 pt inset on each side; compensate so the
        // text column is exactly as wide as the row heights assume.
        time.frame = NSRect(x: padding - 2, y: 0, width: timeWidth + 4, height: LogPalette.lineHeight)
        text.frame = NSRect(x: textX - 2, y: 0, width: bounds.width - textX - padding + 4, height: bounds.height)
        if isFlipped == false {
            time.frame.origin.y = bounds.height - LogPalette.lineHeight
        }
    }
}
