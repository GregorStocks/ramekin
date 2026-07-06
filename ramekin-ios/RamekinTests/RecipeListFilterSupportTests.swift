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
            createdBefore: "2024-02-01",
            photoSizeThreshold: NumericThreshold(comparison: .lessThan, value: 100_000),
            photoDimensionThreshold: NumericThreshold(comparison: .greaterThan, value: 600)
        )

        XCTAssertEqual(
            RecipeListFilterSupport.buildQuery(from: state),
            """
            chicken soup tag:"Quick Dinner" tag:Weeknight source:"NY Times Cooking" \
            has:photos photo_size:<100000 photo_dim:>600 created:2024-01-01..2024-02-01
            """
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

    func testHasTextQueryReflectsFreeTextOnly() {
        func hasText(_ searchText: String) -> Bool {
            RecipeListFilterSupport.hasTextQuery(
                RecipeListFilterState(searchText: searchText, photoFilter: .any)
            )
        }

        // Free text -> rank by relevance (the list sends no sort).
        XCTAssertTrue(hasText("garlic bread"))
        XCTAssertTrue(hasText("\"green beans\""))
        // Whitespace-only is not a text query.
        XCTAssertFalse(hasText("   "))
        // DSL filter tokens typed into the search field are parsed as filters,
        // not text, so they keep browsing by the chosen sort (mirrors the
        // server's parse_query).
        XCTAssertFalse(hasText("tag:dinner"))
        XCTAssertFalse(hasText("created:>2024-01-01"))
        XCTAssertFalse(hasText("has:photos"))
        XCTAssertFalse(hasText("tag:dinner source:nyt"))
        // A DSL token mixed with real text still ranks by relevance.
        XCTAssertTrue(hasText("tag:dinner pasta"))

        // Structured filters (tag chips, source, dates) live outside searchText
        // and are likewise not text terms.
        XCTAssertFalse(
            RecipeListFilterSupport.hasTextQuery(
                RecipeListFilterState(
                    selectedTags: ["Weeknight"],
                    photoFilter: .hasPhotos,
                    source: "Serious Eats",
                    createdAfter: "2024-04-01"
                )
            )
        )
    }

    func testBuildQueryQuotesHierarchicalTagNames() {
        let state = RecipeListFilterState(
            selectedTags: ["ingredient:chicken", "season:winter"],
            photoFilter: .any
        )

        XCTAssertEqual(
            RecipeListFilterSupport.buildQuery(from: state),
            "tag:\"ingredient:chicken\" tag:\"season:winter\""
        )
    }

    func testNumericThresholdParsingAndFormatting() {
        XCTAssertEqual(
            RecipeListFilterSupport.numericThreshold(from: "<100000"),
            NumericThreshold(comparison: .lessThan, value: 100_000)
        )
        XCTAssertEqual(
            RecipeListFilterSupport.numericThreshold(from: ">600"),
            NumericThreshold(comparison: .greaterThan, value: 600)
        )
        XCTAssertNil(RecipeListFilterSupport.numericThreshold(from: "600"))
        XCTAssertNil(RecipeListFilterSupport.numericThreshold(from: ">abc"))

        XCTAssertEqual(
            RecipeListFilterSupport.thresholdQueryValue(
                isEnabled: true,
                comparison: .lessThan,
                value: " 500 "
            ),
            "<500"
        )
        XCTAssertEqual(
            RecipeListFilterSupport.thresholdQueryValue(
                isEnabled: false,
                comparison: .greaterThan,
                value: "600"
            ),
            ""
        )
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
        XCTAssertEqual(
            RecipeListFilterSupport.advancedFilterLabel(
                for: RecipeListFilterState(
                    photoSizeThreshold: NumericThreshold(comparison: .lessThan, value: 100_000)
                )
            ),
            "Advanced"
        )
    }
}
