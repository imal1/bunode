//! Bunode's process-management core.
//!
//! This crate will hold the Supervisor and everything it needs: Run state
//! machines, Process Discovery, port scanning, the log collector and the
//! Project repository. It must not depend on the UI, on the FFI layer
//! (UniFFI) or on any Apple framework, so that it can later be hosted by a
//! standalone Supervisor daemon unchanged (ADR-0001, ADR-0002).

/// The version of `bunode-core`, as set in its `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "expected MAJOR.MINOR.PATCH, got {}",
            version()
        );
        assert!(
            parts.iter().all(|p| p.parse::<u32>().is_ok()),
            "expected numeric parts, got {}",
            version()
        );
    }
}
