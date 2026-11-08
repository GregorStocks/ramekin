import XCTest
@testable import Ramekin

@MainActor
final class AddSheetFlowTests: XCTestCase {
    private struct Refused: LocalizedError {
        var errorDescription: String? { "Sign in to plan meals." }
    }

    func testASuccessfulAddConfirmsAnnouncesAndDismisses() async {
        var announced: [String] = []
        let flow = AddSheetFlow(delayNanoseconds: 1_000_000) { announced.append($0) }
        let dismissed = expectation(description: "dismissed")
        var adds = 0

        flow.submit(confirmation: "Added 3 items", dismiss: { dismissed.fulfill() }, add: { adds += 1 })

        XCTAssertEqual(flow.phase, .confirmed("Added 3 items"))
        XCTAssertEqual(flow.confirmation, "Added 3 items")
        XCTAssertNil(flow.error)
        XCTAssertEqual(announced, ["Added 3 items"])
        XCTAssertEqual(adds, 1)
        await fulfillment(of: [dismissed], timeout: 2)
    }

    func testAFailedAddShowsWhyAndCanBeRetried() {
        let flow = AddSheetFlow(delayNanoseconds: 1_000_000) { _ in }
        var shouldFail = true

        flow.submit(confirmation: "Added to meal plan", dismiss: {}, add: {
            if shouldFail { throw Refused() }
        })
        XCTAssertEqual(flow.phase, .failed("Sign in to plan meals."))
        XCTAssertFalse(flow.isConfirmed)

        shouldFail = false
        flow.submit(confirmation: "Added to meal plan", dismiss: {}, add: {
            if shouldFail { throw Refused() }
        })
        XCTAssertEqual(flow.phase, .confirmed("Added to meal plan"))
        flow.cancel()
    }

    func testASecondSubmitAfterConfirmingDoesNothing() {
        let flow = AddSheetFlow(delayNanoseconds: 1_000_000_000) { _ in }
        var adds = 0

        flow.submit(confirmation: "Added 2 items", dismiss: {}, add: { adds += 1 })
        flow.submit(confirmation: "Added 5 items", dismiss: {}, add: { adds += 1 })

        XCTAssertEqual(adds, 1)
        XCTAssertEqual(flow.confirmation, "Added 2 items")
        flow.cancel()
    }

    func testCancellingDropsThePendingDismissal() async throws {
        let flow = AddSheetFlow(delayNanoseconds: 1_000_000) { _ in }
        var dismissed = false

        flow.submit(confirmation: "Added to meal plan", dismiss: { dismissed = true }, add: {})
        flow.cancel()
        try await Task.sleep(nanoseconds: 50_000_000)

        XCTAssertFalse(dismissed)
    }
}
