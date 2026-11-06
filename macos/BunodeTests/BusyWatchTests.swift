import Foundation
import Testing
@testable import Bunode

struct BusyWatchTests {
    let start = Date(timeIntervalSince1970: 1_000)

    @Test func flagsServicesBusyForTooLong() {
        var watch = BusyWatch()
        watch.update(busy: [1, 2], now: start)
        watch.update(busy: [1, 2], now: start.addingTimeInterval(4))
        #expect(watch.overdue(now: start.addingTimeInterval(9), timeout: 10).isEmpty)
        #expect(watch.overdue(now: start.addingTimeInterval(11), timeout: 10) == [1, 2])
    }

    @Test func forgetsServicesThatMovedOn() {
        var watch = BusyWatch()
        watch.update(busy: [1, 2], now: start)
        watch.update(busy: [2], now: start.addingTimeInterval(5))
        #expect(watch.overdue(now: start.addingTimeInterval(11), timeout: 10) == [2])
        // Busy again later: the clock starts over.
        watch.update(busy: [1, 2], now: start.addingTimeInterval(12))
        #expect(watch.overdue(now: start.addingTimeInterval(13), timeout: 10) == [2])
        watch.update(busy: [], now: start.addingTimeInterval(14))
        #expect(watch.isEmpty)
    }
}
