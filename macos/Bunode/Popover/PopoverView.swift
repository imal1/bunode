import SwiftUI

/// The popover's root view (design-handoff §2.2): a fixed header, the
/// content area and the fixed bottom menu. For now the content area only
/// shows the version of the Rust core, which proves the SwiftUI → UniFFI →
/// Rust path works; the Project and Service list replaces it.
struct PopoverView: View {
    let model: AppModel
    let openSettings: (SettingsPane?, Bool) -> Void
    let quit: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            content
            Divider()
                .padding(.horizontal, 8)
                .padding(.top, 6)
                .padding(.bottom, 4)
            MenuRow(title: "Settings…", key: ",", action: { openSettings(nil, false) })
            MenuRow(title: "Quit Bunode", key: "q", action: quit)
        }
        .padding(6)
        .frame(width: 340)
    }

    private var header: some View {
        Text("Bunode")
            .font(.body.weight(.semibold))
            .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
            .padding(EdgeInsets(top: 4, leading: 10, bottom: 6, trailing: 2))
            .accessibilityAddTraits(.isHeader)
    }

    private var content: some View {
        Text("Rust core \(Core.version)")
            .font(.caption)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 30)
    }
}

/// A row of the popover's bottom menu: the title on the left, its ⌘
/// shortcut on the right.
private struct MenuRow: View {
    let title: LocalizedStringKey
    let key: Character
    let action: () -> Void

    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack {
                Text(title)
                Spacer()
                Text(verbatim: "⌘" + String(key).uppercased())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
            }
            .padding(.horizontal, 10)
            .frame(minHeight: 28)
            .contentShape(Rectangle())
            .background(
                isHovered ? Color(nsColor: .quaternarySystemFill) : .clear,
                in: RoundedRectangle(cornerRadius: 7)
            )
        }
        .buttonStyle(.plain)
        .keyboardShortcut(KeyEquivalent(key), modifiers: .command)
        .onHover { hovering in
            withAnimation(.easeOut(duration: 0.1)) {
                isHovered = hovering
            }
        }
    }
}

#Preview {
    PopoverView(
        model: AppModel(dataDirectory: URL.temporaryDirectory.appending(path: "BunodePreview")),
        openSettings: { _, _ in },
        quit: {}
    )
}
