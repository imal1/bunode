import AppKit

/// System open panels, shown as a sheet on the given window.
@MainActor
enum Panels {
    static func chooseFolder(prompt: String, startingAt directory: URL? = nil, in window: NSWindow?) async -> URL? {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = false
        panel.prompt = prompt
        panel.directoryURL = directory
        return await run(panel, in: window)
    }

    static func run(_ panel: NSOpenPanel, in window: NSWindow?) async -> URL? {
        let response: NSApplication.ModalResponse
        if let window {
            response = await panel.beginSheetModal(for: window)
        } else {
            NSApp.activate()
            response = panel.runModal()
        }
        return response == .OK ? panel.url : nil
    }
}
