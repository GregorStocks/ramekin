import XCTest
@testable import Ramekin

final class UniversalLinkTests: XCTestCase {
    func testRecipeLinkSetsPendingRecipeId() throws {
        let id = UUID()
        let appState = AppState()

        appState.handleUniversalLink(try XCTUnwrap(URL(string: "https://ramekin.app/recipes/\(id)")))

        XCTAssertEqual(appState.pendingRecipeId, id)
    }

    func testRecipeLinkAllowsQueryStringAndTrailingSlash() throws {
        let id = UUID()
        let appState = AppState()

        appState.handleUniversalLink(try XCTUnwrap(URL(string: "https://ramekin.app/recipes/\(id)/?source=pdf")))

        XCTAssertEqual(appState.pendingRecipeId, id)
    }

    func testNonRecipeLinkIsIgnored() throws {
        let appState = AppState()

        appState.handleUniversalLink(try XCTUnwrap(URL(string: "https://ramekin.app/meal-plan")))

        XCTAssertNil(appState.pendingRecipeId)
    }
}
