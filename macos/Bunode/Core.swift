/// The app's way into the Rust core. Views and controllers go through this
/// instead of calling the generated UniFFI bindings (`bunode_ffi.swift`)
/// directly, so the FFI surface stays in one place.
struct Core {
    /// The version of the Rust core linked into the app, such as "0.1.0".
    var version: String {
        coreVersion()
    }
}
