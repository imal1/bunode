import AppKit
import SwiftUI

/// Short confirmations at the bottom of a window (design-handoff §6.4).
/// Each window has its own; a new message replaces the current one.
@MainActor
@Observable
final class Toaster {
    private(set) var message: String?
    @ObservationIgnored private var hideTask: Task<Void, Never>?

    func show(_ message: String) {
        self.message = message
        NSAccessibility.post(
            element: NSApp as Any,
            notification: .announcementRequested,
            userInfo: [
                .announcement: message,
                .priority: NSAccessibilityPriorityLevel.high.rawValue,
            ]
        )
        hideTask?.cancel()
        hideTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(2.4))
            guard !Task.isCancelled else { return }
            self?.message = nil
        }
    }
}

extension View {
    /// Shows the toaster's message centered at the bottom of this view.
    func toasts(_ toaster: Toaster, maxWidth: CGFloat) -> some View {
        overlay(alignment: .bottom) {
            ZStack {
                if let message = toaster.message {
                    Text(message)
                        .font(.callout)
                        .foregroundStyle(.white)
                        .multilineTextAlignment(.center)
                        .padding(.vertical, 7)
                        .padding(.horizontal, 14)
                        .background(Color(white: 0.11, opacity: 0.92), in: RoundedRectangle(cornerRadius: 9))
                        .frame(maxWidth: maxWidth)
                        .padding(.bottom, 16)
                        .transition(.opacity)
                        .accessibilityAddTraits(.updatesFrequently)
                }
            }
            .animation(.easeOut(duration: 0.15), value: toaster.message)
            .allowsHitTesting(false)
        }
    }
}
