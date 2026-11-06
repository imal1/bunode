# Supervisor runs inside the app process for the MVP

The Supervisor runs in the app process, not as a separate daemon. Closing the main window keeps Runs alive because the menu bar app stays up; quitting Bunode asks for confirmation and then stops every Run. A standalone daemon was rejected for the MVP because of the extra IPC and lifecycle work. To keep that door open, the Supervisor lives in a Rust crate with no dependency on the UI or the FFI layer, so it can later be hosted by a launchd daemon unchanged.
