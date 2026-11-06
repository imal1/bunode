import SwiftUI

/// The popover's root view (design-handoff §2.2): the current view of the
/// stack, then the fixed bottom menu.
struct PopoverView: View {
    let model: AppModel
    let router: PopoverRouter
    let toaster: Toaster
    let openSettings: (SettingsPane?, Bool) -> Void
    let quit: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ZStack(alignment: .top) {
                screen
                    .id(router.screen)
                    .transition(.push(from: router.isForward ? .trailing : .leading))
            }
            .clipped()
            Divider()
                .padding(.horizontal, 8)
                .padding(.top, 6)
                .padding(.bottom, 4)
            MenuRow(title: "Settings…", key: ",", action: { openSettings(nil, false) })
            MenuRow(title: "Quit Bunode", key: "q", action: quit)
        }
        .padding(6)
        .frame(width: 340)
        .toasts(toaster, maxWidth: 380)
    }

    @ViewBuilder
    private var screen: some View {
        switch router.screen {
        case .home:
            HomeView(model: model, router: router, openSettings: openSettings)
        case .addService(let projectID):
            AddServiceView(model: model, projectID: projectID, router: router, toaster: toaster)
        case .service(let serviceID):
            ServiceView(model: model, serviceID: serviceID, router: router, toaster: toaster)
        case .logs(let serviceID):
            LogsView(model: model, serviceID: serviceID, router: router, toaster: toaster)
        }
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
