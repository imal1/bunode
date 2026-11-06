# Native SwiftUI/AppKit UI over an in-process Rust core, no WebView

The requirements suggested Tauri 2 + React, but a WebView could not meet the performance bar we want (idle memory, menu bar responsiveness, log rendering), so the UI is SwiftUI with AppKit where performance matters (log view, status item). Process management lives in a Rust core linked into the app over FFI. We chose this over pure Swift because the Rust core stays independent of the UI and of Apple frameworks: it can later become a standalone Supervisor daemon, a CLI or an MCP server, and it reuses mature Rust crates for process groups, system info and ANSI parsing.

## Considered Options

- **Tauri 2 + React**: rejected, WebView overhead.
- **Pure Swift (SwiftUI + Darwin APIs)**: simplest build and debugging, but ties the core to the app and to Swift.
- **Rust-native GUI (GPUI, iced, Slint)**: fast, but not macOS-native controls; menu bar and accessibility would be hand-built.
- **React Native macOS**: native views but still a JS runtime.

## Consequences

Two toolchains (cargo + Xcode), universal builds need both Rust targets merged, and the FFI boundary is where async calls, log streaming and backpressure must be designed carefully.
