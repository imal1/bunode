import Foundation
import Testing
@testable import Bunode

/// Counts go through the String Catalog's plural rules, never through
/// string concatenation (design-handoff §7).
struct LocalizationTests {
    @Test func servicesArePluralized() {
        #expect(String(localized: "\(1) Services") == "1 Service")
        #expect(String(localized: "\(2) Services") == "2 Services")
        #expect(String(localized: "\(0) Services") == "0 Services")
    }
}
