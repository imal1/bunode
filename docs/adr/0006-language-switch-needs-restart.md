# The language setting applies after a restart

Bunode ships English and Simplified Chinese. Settings › General › Language offers System, English and 简体中文. Choosing one writes the app's own `AppleLanguages` default, or removes it for System. The change takes effect the next time Bunode launches, and the setting row offers Restart Now. Restarting quits through the usual confirmation, because quitting stops every Run (ADR-0003); a short-lived `sh -c` reopens the app: it waits about a second until Bunode's process is gone, then runs `open` with the app's own path, not its bundle id, which could pick another copy. All UI text lives in the String Catalog. The Rust core produces no UI text: it returns typed errors (an enum with parameters) that Swift localizes. Logs, commands and paths are never translated.

## Considered Options

- **Switch live with an in-app locale override**: rejected; SwiftUI's `\.locale` environment does not change `String(localized:)`, AppKit menus, `NSAlert` or system-provided strings, so the UI would show a mix of languages until the next launch anyway, and every view would need the override threaded through it.
- **Follow the system language only**: rejected; users want Bunode in a different language from the rest of macOS.

## Consequences

Switching languages while Services run means stopping them. The restart dialog says so, and the user can postpone the restart and keep working in the old language. Fixed Chinese terms are recorded in GLOSSARY.md.
