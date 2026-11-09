import AppKit
import SwiftUI

// Building blocks shared by the popover's views (design-handoff §3, §4).

extension ProjectState {
    /// The folder a Service runs in: the Project's, or `folder` inside it.
    func runPath(folder: String?) -> String {
        guard let folder else { return path }
        return URL(filePath: path).appending(path: folder).path(percentEncoded: false)
    }
}

/// How a status dot looks. Color is never the only signal: Stopped is a
/// hollow ring, Crashed and Failed a solid gray dot with a note, busy
/// states pulse next to a spinner (design-handoff §9).
enum DotStyle: Equatable {
    case stopped, running, busy, failed
}

struct StatusDot: View {
    let style: DotStyle
    var size: CGFloat = 7

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var dimmed = false

    var body: some View {
        Group {
            switch style {
            case .stopped:
                Circle().strokeBorder(Color(nsColor: .systemGray), lineWidth: 1.5)
            case .running:
                Circle().fill(Color(nsColor: .systemGreen))
            case .failed:
                Circle().fill(Color(nsColor: .systemGray))
            case .busy:
                Circle()
                    .fill(Color(nsColor: .systemOrange))
                    .opacity(dimmed ? 0.25 : 1)
                    .onAppear {
                        guard !reduceMotion else { return }
                        withAnimation(.easeInOut(duration: 0.5).repeatForever(autoreverses: true)) {
                            dimmed = true
                        }
                    }
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// The 32 pt tile at the start of a Service row (design-handoff §5.1). Its
/// color says what the Run is doing; the second line says it in words.
struct StatusTile: View {
    let style: DotStyle

    private var colors: (fill: Color, icon: Color) {
        switch style {
        case .stopped:
            (Color(nsColor: .quaternaryLabelColor), .secondary)
        case .running:
            (Color(light: 0xDDF5E3, dark: NSColor.systemGreen.withAlphaComponent(0.22)),
             Color(light: 0x1B7F3A, highContrast: 0x0E5A26, dark: .systemGreen))
        case .busy:
            (Color(light: 0xFFEFD6, dark: NSColor.systemOrange.withAlphaComponent(0.22)),
             Color(light: 0x9A5B00, highContrast: 0x6B3E00, dark: .systemOrange))
        case .failed:
            (Color(light: 0xFFE3E0, dark: NSColor.systemRed.withAlphaComponent(0.22)), .statusErrorText)
        }
    }

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 9).fill(colors.fill)
            icon.foregroundStyle(colors.icon)
        }
        .frame(width: 32, height: 32)
        .animation(.easeOut(duration: 0.15), value: style)
        .accessibilityHidden(true)
    }

    // The paths are design-handoff §5.1's, in its 16 pt view box.
    @ViewBuilder private var icon: some View {
        switch style {
        case .stopped, .running:
            Path { path in
                path.addLines([CGPoint(x: 3.5, y: 4.5), CGPoint(x: 7, y: 8), CGPoint(x: 3.5, y: 11.5)])
                path.addLines([CGPoint(x: 8.5, y: 12), CGPoint(x: 13, y: 12)])
            }
            .stroke(style: StrokeStyle(lineWidth: 1.7, lineCap: .round, lineJoin: .round))
            .frame(width: 16, height: 16)
        case .busy:
            TimelineView(.animation) { context in
                Circle()
                    .trim(from: 0, to: 0.75)
                    .stroke(style: StrokeStyle(lineWidth: 1.75, lineCap: .round))
                    .frame(width: 10.5, height: 10.5)
                    .rotationEffect(.degrees(context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 0.9) / 0.9 * 360))
            }
        case .failed:
            ZStack {
                Path { path in
                    path.addLines([CGPoint(x: 8, y: 3), CGPoint(x: 8, y: 9)])
                }
                .stroke(style: StrokeStyle(lineWidth: 2, lineCap: .round))
                Path(ellipseIn: CGRect(x: 6.8, y: 11.3, width: 2.4, height: 2.4))
            }
            .frame(width: 16, height: 16)
            .scaleEffect(15.0 / 16)
        }
    }
}

extension Color {
    /// `status-error-text` (design-handoff §3).
    static let statusErrorText = Color(light: 0xB42318, highContrast: 0x8A1810, dark: .systemRed)

    /// A fixed color in Light, optionally darker under Increase Contrast,
    /// and a system color (which adapts by itself) in Dark.
    fileprivate init(light: UInt32, highContrast: UInt32? = nil, dark: NSColor) {
        func color(_ hex: UInt32) -> NSColor {
            NSColor(
                srgbRed: CGFloat(hex >> 16 & 0xFF) / 255,
                green: CGFloat(hex >> 8 & 0xFF) / 255,
                blue: CGFloat(hex & 0xFF) / 255,
                alpha: 1
            )
        }
        self.init(nsColor: NSColor(name: nil) { appearance in
            switch appearance.bestMatch(from: [.aqua, .accessibilityHighContrastAqua, .darkAqua, .accessibilityHighContrastDarkAqua]) {
            case .darkAqua?, .accessibilityHighContrastDarkAqua?: dark
            case .accessibilityHighContrastAqua?: color(highContrast ?? light)
            default: color(light)
            }
        })
    }
}

/// A rounded card of rows separated by hairlines.
struct Card<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Group(subviews: content) { rows in
                ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                    if index > 0 {
                        Divider()
                    }
                    row
                }
            }
        }
        .background(.background.secondary, in: RoundedRectangle(cornerRadius: 10))
        .padding(.horizontal, 2)
    }
}

/// A card row: a label (with an optional help line) and a control.
struct CardRow<Control: View>: View {
    let title: LocalizedStringKey
    var help: LocalizedStringKey?
    @ViewBuilder var control: Control

    var body: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                if let help {
                    Text(help)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            control
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 12)
        .frame(minHeight: 38)
    }
}

/// The top bar of a popover sub-view: Back, a title and a subtitle, and an
/// optional action on the right.
struct TopBar<Subtitle: View, Trailing: View>: View {
    let title: String
    var backLabel: String = String(localized: "Back")
    let onBack: () -> Void
    @ViewBuilder var subtitle: Subtitle
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 6) {
            Button(action: onBack) {
                Image(systemName: "chevron.left")
                    .font(.body.weight(.semibold))
                    .frame(width: 28, height: 28)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.borderless)
            .help(backLabel)
            .accessibilityLabel(backLabel)

            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.body.weight(.semibold))
                    .lineLimit(1)
                    .help(title)
                    .accessibilityAddTraits(.isHeader)
                subtitle
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            trailing
        }
        .padding(EdgeInsets(top: 0, leading: 0, bottom: 8, trailing: 2))
    }
}

extension TopBar where Trailing == EmptyView {
    init(title: String, backLabel: String = String(localized: "Back"), onBack: @escaping () -> Void, @ViewBuilder subtitle: () -> Subtitle) {
        self.init(title: title, backLabel: backLabel, onBack: onBack, subtitle: subtitle, trailing: { EmptyView() })
    }
}

/// The popover's scrollable content area. It is as tall as its content,
/// up to what fits on screen; only then does it scroll.
struct PopoverScroll<Content: View>: View {
    @ViewBuilder var content: Content
    @State private var contentHeight: CGFloat = 0

    /// Screen height minus 40 pt, minus the 156 pt of fixed areas
    /// (design-handoff §2.2).
    static var maxHeight: CGFloat {
        let screen = NSScreen.main?.visibleFrame.height ?? 900
        return max(200, screen - 40 - 156)
    }

    var body: some View {
        ScrollView(.vertical) {
            content
                .onGeometryChange(for: CGFloat.self) { proxy in
                    proxy.size.height
                } action: { height in
                    contentHeight = height
                }
        }
        .scrollBounceBehavior(.basedOnSize)
        .frame(height: min(contentHeight, Self.maxHeight))
    }
}

/// Lays out its children left to right, wrapping to new rows.
struct FlowLayout: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var widest: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > 0, x + size.width > maxWidth {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            x += size.width + spacing
            widest = max(widest, x - spacing)
            rowHeight = max(rowHeight, size.height)
        }
        return CGSize(width: proposal.width ?? widest, height: y + rowHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX
        var y = bounds.minY
        var rowHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > bounds.minX, x + size.width > bounds.maxX {
                x = bounds.minX
                y += rowHeight + spacing
                rowHeight = 0
            }
            subview.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
    }
}

extension View {
    /// The hover background of popover rows (`fill-hover`, 100 ms).
    func hoverHighlight(cornerRadius: CGFloat = 8) -> some View {
        modifier(HoverHighlight(cornerRadius: cornerRadius))
    }
}

private struct HoverHighlight: ViewModifier {
    let cornerRadius: CGFloat
    @State private var isHovered = false

    func body(content: Content) -> some View {
        content
            .background(
                isHovered ? Color(nsColor: .quaternarySystemFill) : .clear,
                in: RoundedRectangle(cornerRadius: cornerRadius)
            )
            .onHover { hovering in
                withAnimation(.easeOut(duration: 0.1)) {
                    isHovered = hovering
                }
            }
    }
}

/// A warning bar with actions (design-handoff §4, InlineNotice): port
/// conflicts, missing folders. Announced once when it appears. The caller
/// sets its margins.
struct InlineNotice<Actions: View>: View {
    let text: String
    var detail: String?
    var verticalPadding: CGFloat = 10
    @ViewBuilder var actions: Actions

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
                Text(text)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .font(.callout)
            if let detail {
                Text(detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack(spacing: 6) {
                Spacer(minLength: 0)
                actions
            }
            .controlSize(.small)
        }
        .padding(.vertical, verticalPadding)
        .padding(.horizontal, 10)
        .background(Color.orange.opacity(0.14), in: RoundedRectangle(cornerRadius: 8))
        .onAppear {
            NSAccessibility.post(
                element: NSApp as Any,
                notification: .announcementRequested,
                userInfo: [.announcement: text, .priority: NSAccessibilityPriorityLevel.high.rawValue]
            )
        }
    }
}
