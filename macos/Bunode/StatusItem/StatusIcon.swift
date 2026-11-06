import AppKit

/// The menu bar icon: a hexagon outline with a colored dot in the middle
/// (design-handoff §2.1).
///
/// The outline is a template image, so it follows the menu bar's color. A
/// template image has only one color, so the dot is a separate view on top
/// (`StatusDotView`).
enum StatusIcon {
    static let size = NSSize(width: 15, height: 15)
    /// The dot has a 2 pt radius.
    static let dotDiameter: CGFloat = 4

    static func hexagon() -> NSImage {
        let image = NSImage(size: size, flipped: true) { rect in
            // Geometry from the design canvas: a 16×16 viewBox drawn at 15×15 pt.
            let scale = rect.width / 16
            let corners: [(x: CGFloat, y: CGFloat)] = [
                (8, 1.8), (13.4, 4.9), (13.4, 11.1), (8, 14.2), (2.6, 11.1), (2.6, 4.9),
            ]
            let path = NSBezierPath()
            for (index, corner) in corners.enumerated() {
                let point = NSPoint(x: corner.x * scale, y: corner.y * scale)
                if index == 0 {
                    path.move(to: point)
                } else {
                    path.line(to: point)
                }
            }
            path.close()
            path.lineWidth = 1.5
            path.lineJoinStyle = .round
            NSColor.black.setStroke()
            path.stroke()
            return true
        }
        image.isTemplate = true
        return image
    }
}

/// The colored dot in the middle of the menu bar hexagon.
final class StatusDotView: NSView {
    var color: NSColor = .systemGray {
        didSet { needsDisplay = true }
    }

    override func draw(_ dirtyRect: NSRect) {
        color.setFill()
        NSBezierPath(ovalIn: bounds).fill()
    }

    /// Clicks go through to the status item button underneath.
    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }
}
