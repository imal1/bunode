import AppKit
import SwiftUI

// Building blocks shared by the popover's views (design-handoff §3, §4).

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

    /// Screen height minus 40 pt (design-handoff §2.2), minus room for the
    /// header and the bottom menu.
    static var maxHeight: CGFloat {
        let screen = NSScreen.main?.visibleFrame.height ?? 900
        return max(200, screen - 40 - 120)
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
