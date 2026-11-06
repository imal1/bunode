import SwiftUI

/// The Settings window's content. Empty for now: General, Projects and
/// Advanced come with the tickets that need them.
struct SettingsView: View {
    var body: some View {
        Color.clear
            .frame(
                width: SettingsWindowController.contentSize.width,
                height: SettingsWindowController.contentSize.height
            )
    }
}

#Preview {
    SettingsView()
}
