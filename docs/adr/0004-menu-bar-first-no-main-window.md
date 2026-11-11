# Menu bar popover is the only main UI; no main window in the MVP

The requirements described a main window with Dashboard, Projects, Processes and Logs pages, and a Dock icon that comes and goes with that window. The design instead puts everything a user does day to day in the menu bar popover: the Project and Service list with one-click start and stop, each Service's settings, its logs, adding a Service, and the list of Discovered Processes. A separate Settings window (⌘,) holds only global preferences and Project registration. We chose this because the typical session is "who is running, on which port, what crashed", which fits one popover screen and two clicks, and because a second full-size surface would duplicate every view and its state handling.

Bunode runs as an agent app (`LSUIElement`): no Dock icon and no app menu bar, ever. The Settings window is a normal window that comes to the front when opened. (Amended: the app menu bar and a Dock icon appear while the Settings window is open, [ADR-0009](0009-app-menu-bar-while-settings-is-open.md).)

## Considered Options

- **Main window plus menu bar (as in the requirements)**: rejected for the MVP; it doubles the UI surface and the testing work, and the menu bar already covers the core tasks.
- **Popover for quick actions, window for setup**: rejected; Service settings live next to the Service row, so configuring and running stay in one place.

## Consequences

The popover is fixed at 340 pt wide, so dense data from the requirements (CPU and memory columns, process trees, logs side by side) does not fit and moves to a later phase. Logs are viewed one Service at a time inside the popover (superseded: they open in Console, [ADR-0008](0008-logs-open-in-console.md)). If a main window comes back later, it reads the same Supervisor state and can reuse the popover's views, and this ADR should be superseded.
