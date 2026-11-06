import Testing
@testable import Bunode

/// Calls through the real UniFFI bindings into the Rust core, so a broken
/// link or a bindings/library mismatch fails here.
struct CoreTests {
    @Test func versionComesFromRust() {
        let parts = Core.version.split(separator: ".")
        #expect(parts.count == 3)
        #expect(parts.allSatisfy { Int($0) != nil })
    }
}
