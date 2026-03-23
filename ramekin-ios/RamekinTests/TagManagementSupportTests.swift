import XCTest
@testable import Ramekin

final class TagManagementSupportTests: XCTestCase {
    func testNormalizedNameTrimsWhitespace() {
        XCTAssertEqual(
            TagManagementSupport.normalizedName(from: "  Dinner  "),
            "Dinner"
        )
    }

    func testNormalizedNameRejectsEmptyValues() {
        XCTAssertNil(TagManagementSupport.normalizedName(from: "   "))
        XCTAssertNil(TagManagementSupport.normalizedName(from: "\n\t"))
    }

    func testRecipeCountTextPluralizes() {
        XCTAssertEqual(TagManagementSupport.recipeCountText(for: 1), "1 recipe")
        XCTAssertEqual(TagManagementSupport.recipeCountText(for: 4), "4 recipes")
    }

    func testRenamedTagsUpdatesNameAndMaintainsSortOrder() {
        let breakfastId = UUID()
        let dinnerId = UUID()
        let tags = [
            TagItem(createdAt: Date(), id: dinnerId, name: "Dinner", recipeCount: 3),
            TagItem(createdAt: Date(), id: breakfastId, name: "Breakfast", recipeCount: 1)
        ]

        let renamed = TagManagementSupport.renamedTags(tags, id: dinnerId, newName: "Appetizer")

        XCTAssertEqual(renamed.map(\.name), ["Appetizer", "Breakfast"])
        XCTAssertEqual(renamed.first?.recipeCount, 3)
    }

    func testRemovingTagDropsOnlyMatchingTag() {
        let firstId = UUID()
        let secondId = UUID()
        let tags = [
            TagItem(createdAt: Date(), id: firstId, name: "Dinner", recipeCount: 3),
            TagItem(createdAt: Date(), id: secondId, name: "Quick", recipeCount: 2)
        ]

        let filtered = TagManagementSupport.removingTag(tags, id: firstId)

        XCTAssertEqual(filtered.map(\.id), [secondId])
        XCTAssertEqual(filtered.map(\.name), ["Quick"])
    }

    func testAPIErrorFormatterPrefersServerErrorMessage() {
        let payload = Data(#"{"error":"Tag with that name already exists"}"#.utf8)
        let error = ErrorResponse.error(
            409,
            payload,
            nil,
            DecodableRequestBuilderError.unsuccessfulHTTPStatusCode
        )

        XCTAssertEqual(
            APIErrorFormatter.userMessage(from: error, fallback: "Fallback"),
            "Tag with that name already exists"
        )
    }

    func testAPIErrorFormatterFallsBackToUnderlyingError() {
        let underlyingError = NSError(
            domain: "TagManagementTests",
            code: 42,
            userInfo: [NSLocalizedDescriptionKey: "Request timed out"]
        )
        let error = ErrorResponse.error(500, Data("not-json".utf8), nil, underlyingError)

        XCTAssertEqual(
            APIErrorFormatter.userMessage(from: error, fallback: "Fallback"),
            "Request timed out"
        )
    }
}
