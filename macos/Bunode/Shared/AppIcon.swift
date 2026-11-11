import AppKit

/// Bunode's icon as a plain bitmap. On macOS 26 the About panel and NSAlert
/// look the app's Icon Composer icon up themselves and get a blank
/// placeholder, so both are handed this image instead. Icon Services renders
/// it, as Finder does, in the current light or dark appearance.
@MainActor
enum AppIcon {
    static let image: NSImage = {
        let icon = NSWorkspace.shared.icon(forFile: Bundle.main.bundlePath)
        let pixels = 512
        guard let rep = NSBitmapImageRep(
            bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
        ) else { return icon }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        icon.draw(in: NSRect(x: 0, y: 0, width: pixels, height: pixels))
        NSGraphicsContext.restoreGraphicsState()
        rep.size = NSSize(width: 256, height: 256)
        let image = NSImage(size: rep.size)
        image.addRepresentation(rep)
        return image
    }()
}

extension NSAlert {
    /// An alert that shows Bunode's icon rather than the blank placeholder.
    @MainActor
    static func withAppIcon() -> NSAlert {
        let alert = NSAlert()
        alert.icon = AppIcon.image
        return alert
    }
}
