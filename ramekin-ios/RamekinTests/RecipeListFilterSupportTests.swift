import XCTest
@testable import Ramekin

final class RecipeListFilterSupportTests: XCTestCase {
    func testBuildQueryIncludesSourceTagsPhotosAndDateRange() {
        let state = RecipeListFilterState(
            searchText: "chicken soup",
            selectedTags: ["Weeknight", "Quick Dinner"],
            photoFilter: .hasPhotos,
            source: "NY Times Cooking",
            createdAfter: "2024-01-01",
            createdBefore: "2024-02-01"
        )

        XCTAssertEqual(
            RecipeListFilterSupport.buildQuery(from: state),
            "chicken soup tag:\"Quick Dinner\" tag:Weeknight source:\"NY Times Cooking\" has:photos created:2024-01-01..2024-02-01"
        )
    }

    func testBuildQueryUsesExactCreatedDateWhenBoundsMatch() {
        let state = RecipeListFilterState(
            photoFilter: .any,
            source: "Serious Eats",
            createdAfter: "2024-03-15",
            createdBefore: "2024-03-15"
        )

        XCTAssertEqual(
            RecipeListFilterSupport.buildQuery(from: state),
            "source:\"Serious Eats\" created:2024-03-15"
        )
    }

    func testBuildQueryUsesOpenEndedCreatedFilters() {
        let afterOnlyState = RecipeListFilterState(
            photoFilter: .noPhotos,
            createdAfter: "2024-04-01"
        )
        let beforeOnlyState = RecipeListFilterState(createdBefore: "2024-05-01")

        XCTAssertEqual(
            RecipeListFilterSupport.buildQuery(from: afterOnlyState),
            "no:photos created:>2024-04-01"
        )
        XCTAssertEqual(
            RecipeListFilterSupport.buildQuery(from: beforeOnlyState),
            "created:<2024-05-01"
        )
    }

    func testBuildQueryReturnsNilForEmptyFilters() {
        XCTAssertNil(RecipeListFilterSupport.buildQuery(from: RecipeListFilterState()))
    }

    func testAdvancedFilterLabelUsesSourceOrDateSummary() {
        XCTAssertEqual(
            RecipeListFilterSupport.advancedFilterLabel(
                for: RecipeListFilterState(source: "NYTimes")
            ),
            "NYTimes"
        )
        XCTAssertEqual(
            RecipeListFilterSupport.advancedFilterLabel(
                for: RecipeListFilterState(createdAfter: "2024-01-15")
            ),
            "After Jan 15"
        )
        XCTAssertEqual(
            RecipeListFilterSupport.advancedFilterLabel(
                for: RecipeListFilterState(
                    source: "Bon Appetit",
                    createdAfter: "2024-01-15"
                )
            ),
            "Advanced"
        )
    }
}
