import XCTest
@testable import Ramekin

@MainActor
final class NetworkMonitorTests: XCTestCase {
    func testEveryObserverHearsEachPathChange() {
        let monitor = NetworkMonitor(startMonitoring: false)
        var first: [Bool] = []
        var second: [Bool] = []
        monitor.observe { first.append($0) }
        monitor.observe { second.append($0) }

        monitor.pathChanged(isOnline: false)
        monitor.pathChanged(isOnline: true)

        XCTAssertEqual(first, [false, true])
        XCTAssertEqual(second, [false, true])
        XCTAssertTrue(monitor.isOnline)
    }
}
