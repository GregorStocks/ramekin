import XCTest
@testable import Ramekin

final class SheetAutoDismissSupportTests: XCTestCase {
    private let delay: UInt64 = 50_000_000

    @MainActor
    func testScheduleDismissesAfterDelay() async throws {
        var dismissCount = 0
        var dismissTask: Task<Void, Never>?

        SheetAutoDismissSupport.schedule(&dismissTask, delayNanoseconds: delay) {
            dismissCount += 1
        }

        XCTAssertNotNil(dismissTask)
        XCTAssertEqual(dismissCount, 0)

        await dismissTask?.value

        XCTAssertEqual(dismissCount, 1)
    }

    @MainActor
    func testCancelPreventsDismissal() async throws {
        var dismissCount = 0
        var dismissTask: Task<Void, Never>?

        SheetAutoDismissSupport.schedule(&dismissTask, delayNanoseconds: delay) {
            dismissCount += 1
        }
        let scheduledTask = try XCTUnwrap(dismissTask)

        SheetAutoDismissSupport.cancel(&dismissTask)

        XCTAssertNil(dismissTask)
        XCTAssertTrue(scheduledTask.isCancelled)
        await scheduledTask.value
        XCTAssertEqual(dismissCount, 0)
    }

    @MainActor
    func testRescheduleCancelsEarlierDismissal() async throws {
        var dismissals: [String] = []
        var dismissTask: Task<Void, Never>?

        SheetAutoDismissSupport.schedule(&dismissTask, delayNanoseconds: delay) {
            dismissals.append("first")
        }
        let firstTask = try XCTUnwrap(dismissTask)

        SheetAutoDismissSupport.schedule(&dismissTask, delayNanoseconds: delay) {
            dismissals.append("second")
        }

        XCTAssertTrue(firstTask.isCancelled)
        await firstTask.value
        await dismissTask?.value
        XCTAssertEqual(dismissals, ["second"])
    }
}
