import XCTest
@testable import Ramekin

final class RecipeFormDraftFlowTests: XCTestCase {
    private let draft = RecipeFormDraft(mode: .create, form: RecipeFormData(), recipeText: "Pancakes")

    func testOfferShowsStoredDraftOnly() {
        var flow = RecipeFormDraftFlow.closed
        flow.offer(nil)
        XCTAssertEqual(flow, .closed)

        flow.offer(draft)
        XCTAssertEqual(flow.offeredDraft, draft)
        XCTAssertFalse(flow.isOpen)
    }

    func testResumeOpensFormWithOfferedDraft() {
        var flow = RecipeFormDraftFlow.offered(draft)
        flow.resume()

        XCTAssertTrue(flow.isOpen)
        XCTAssertEqual(flow.resumingDraft, draft)
        XCTAssertNil(flow.offeredDraft)
    }

    func testFreshFormReplacesOfferedDraft() {
        var flow = RecipeFormDraftFlow.offered(draft)
        flow.openFresh()

        XCTAssertTrue(flow.isOpen)
        XCTAssertNil(flow.resumingDraft)
        XCTAssertNil(flow.offeredDraft)
    }

    func testDiscardDropsOffer() {
        var flow = RecipeFormDraftFlow.offered(draft)
        flow.discard()
        XCTAssertEqual(flow, .closed)
    }

    func testOfferWhileOpenKeepsTheOpenForm() {
        var flow = RecipeFormDraftFlow.open(resuming: nil)
        flow.offer(draft)
        XCTAssertEqual(flow, .open(resuming: nil))
    }

    func testCloseEndsOpenFormOnly() {
        var open = RecipeFormDraftFlow.open(resuming: draft)
        open.close()
        XCTAssertEqual(open, .closed)

        var offered = RecipeFormDraftFlow.offered(draft)
        offered.close()
        XCTAssertEqual(offered, .offered(draft))
    }

    func testResumeWithoutOfferDoesNothing() {
        var flow = RecipeFormDraftFlow.closed
        flow.resume()
        XCTAssertEqual(flow, .closed)
    }
}
