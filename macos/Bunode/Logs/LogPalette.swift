import AppKit

/// Log colors (design-handoff §3.1). The log area is dark in both
/// appearances, so these are fixed colors, not semantic ones.
@MainActor
enum LogPalette {
    static let background = NSColor(srgbRed: 0x1C / 255, green: 0x1C / 255, blue: 0x1E / 255, alpha: 1)
    static let text = NSColor(srgbRed: 0xE5 / 255, green: 0xE5 / 255, blue: 0xEA / 255, alpha: 1)
    static let time = NSColor(srgbRed: 0x8E / 255, green: 0x8E / 255, blue: 0x93 / 255, alpha: 1)
    static let system = NSColor(srgbRed: 0x98 / 255, green: 0x98 / 255, blue: 0x9D / 255, alpha: 1)
    static let error = NSColor(srgbRed: 0xFF / 255, green: 0x69 / 255, blue: 0x61 / 255, alpha: 1)

    static let font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
    static let boldFont = NSFont.monospacedSystemFont(ofSize: 11, weight: .bold)
    /// 11 pt at a 1.6 line height.
    static let lineHeight: CGFloat = 17.6

    /// The width of one character of the log font.
    static let characterWidth: CGFloat = ("0" as NSString).size(withAttributes: [.font: font]).width

    /// The 16 basic ANSI colors, tuned to read on the dark background.
    private static let basic: [NSColor] = [
        (0x5C, 0x5C, 0x63), (0xFF, 0x69, 0x61), (0x5F, 0xD0, 0x7A), (0xFF, 0xD6, 0x0A),
        (0x64, 0xA8, 0xFF), (0xDA, 0x8F, 0xFF), (0x5A, 0xD8, 0xE6), (0xE5, 0xE5, 0xEA),
        (0x8E, 0x8E, 0x93), (0xFF, 0x8A, 0x84), (0x7E, 0xE0, 0x95), (0xFF, 0xE0, 0x4D),
        (0x8C, 0xBE, 0xFF), (0xE4, 0xAA, 0xFF), (0x7F, 0xE4, 0xEF), (0xFF, 0xFF, 0xFF),
    ].map { NSColor(srgbRed: CGFloat($0.0) / 255, green: CGFloat($0.1) / 255, blue: CGFloat($0.2) / 255, alpha: 1) }

    static func color(_ color: LogColor) -> NSColor {
        switch color {
        case .rgb(let red, let green, let blue):
            return NSColor(srgbRed: CGFloat(red) / 255, green: CGFloat(green) / 255, blue: CGFloat(blue) / 255, alpha: 1)
        case .palette(let index):
            return palette(Int(index))
        }
    }

    /// The xterm 256-color palette.
    static func palette(_ index: Int) -> NSColor {
        if index < 16 {
            return basic[index]
        }
        if index < 232 {
            let cube = index - 16
            let level = { (v: Int) -> CGFloat in v == 0 ? 0 : CGFloat(55 + v * 40) / 255 }
            return NSColor(srgbRed: level(cube / 36), green: level((cube / 6) % 6), blue: level(cube % 6), alpha: 1)
        }
        let gray = CGFloat(8 + (index - 232) * 10) / 255
        return NSColor(srgbRed: gray, green: gray, blue: gray, alpha: 1)
    }

    static func baseColor(of stream: LogStream) -> NSColor {
        switch stream {
        case .stdout: text
        case .stderr: error
        case .system: system
        }
    }

    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "HH:mm:ss"
        return formatter
    }()

    /// `HH:mm:ss`, 24-hour (design-handoff §6.5).
    static func time(of line: LogLine) -> String {
        timeFormatter.string(from: Date(timeIntervalSince1970: TimeInterval(line.timeMs) / 1000))
    }

    private static let paragraph: NSParagraphStyle = {
        let style = NSMutableParagraphStyle()
        style.minimumLineHeight = lineHeight
        style.maximumLineHeight = lineHeight
        style.lineBreakMode = .byCharWrapping
        return style
    }()

    /// The time column, on the same line grid as the text.
    static func attributedTime(_ time: String) -> NSAttributedString {
        NSAttributedString(string: time, attributes: [
            .font: font,
            .foregroundColor: self.time,
            .paragraphStyle: paragraph,
        ])
    }

    /// The line's text with its ANSI styles applied.
    static func attributedText(of line: LogLine) -> NSAttributedString {
        let text = NSMutableAttributedString(string: line.text, attributes: [
            .font: font,
            .foregroundColor: baseColor(of: line.stream),
            .paragraphStyle: paragraph,
        ])
        let length = text.length
        for span in line.spans {
            let start = Int(span.start)
            guard start < length else { continue }
            let range = NSRange(location: start, length: min(Int(span.length), length - start))
            var attributes: [NSAttributedString.Key: Any] = [:]
            if let foreground = span.style.foreground {
                attributes[.foregroundColor] = color(foreground)
            }
            if let background = span.style.background {
                attributes[.backgroundColor] = color(background)
            }
            if span.style.bold {
                attributes[.font] = boldFont
            }
            if span.style.dim {
                let base = (attributes[.foregroundColor] as? NSColor) ?? baseColor(of: line.stream)
                attributes[.foregroundColor] = base.withAlphaComponent(0.6)
            }
            if span.style.underline {
                attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue
            }
            text.addAttributes(attributes, range: range)
        }
        return text
    }

    /// How many lines `text` takes when wrapped at `columns` characters.
    /// The log font is monospaced, so this is counting, not measuring.
    static func wrappedLineCount(of text: String, columns: Int) -> Int {
        guard columns > 0 else { return 1 }
        var width = 0
        for scalar in text.unicodeScalars {
            width += displayWidth(of: scalar)
        }
        return max(1, (width + columns - 1) / columns)
    }

    /// 2 for East Asian wide characters and emoji, 0 for combining marks.
    static func displayWidth(of scalar: Unicode.Scalar) -> Int {
        let value = scalar.value
        if scalar.properties.generalCategory == .nonspacingMark || value == 0x200D || (0xFE00...0xFE0F).contains(value) {
            return 0
        }
        let wide: [ClosedRange<UInt32>] = [
            0x1100...0x115F, 0x2E80...0xA4CF, 0xAC00...0xD7A3, 0xF900...0xFAFF,
            0xFE30...0xFE4F, 0xFF00...0xFF60, 0xFFE0...0xFFE6, 0x1F300...0x1F64F,
            0x1F900...0x1F9FF, 0x20000...0x3FFFD,
        ]
        return wide.contains { $0.contains(value) } ? 2 : 1
    }

    /// `HH:mm:ss  text` lines, for Copy.
    static func plainText(of lines: some Sequence<LogLine>) -> String {
        lines.map { "\(time(of: $0))  \($0.text)" }.joined(separator: "\n")
    }
}
