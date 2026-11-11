# The app menu bar and a Dock icon while Settings is open

ADR-0004 kept Bunode an agent app with no app menu bar, ever. That left the Settings window as the one window on the Mac with no menu bar of its own: no About Bunode to see which version is installed, no Hide, no Help, and ⌘Tab could not find it. So while the Settings window is open Bunode switches to a regular app (`NSApp.setActivationPolicy(.regular)`), which brings the standard menu bar (Bunode, Edit, Window, Help) and a Dock icon; when the window closes it switches back to `.accessory`. The popover alone never changes the policy.

## Considered Options

- **About in the popover footer instead**: rejected; the footer already shows the version, and a Mac user looks for About in the app menu.
- **Always a regular app**: rejected; a Dock icon for a menu bar utility is what ADR-0004 set out to avoid.

## Consequences

The Dock icon comes and goes with the Settings window, as it does for most menu bar utilities. Release Notes in the Help menu opens the GitHub release of the running version, so every version needs a tag and a CHANGELOG.md section (`scripts/version.sh`). Amends the "no app menu bar, ever" part of ADR-0004.
