import XCTest
@testable import Ramekin

final class RecipeSummaryCacheSupportTests: XCTestCase {
    func testFiltersBySearchTagsPhotosAndCreatedDate() {
        let matching = makeRecipe(
            title: "Chicken Soup",
            description: "Weeknight dinner",
            tags: ["Dinner", "Quick"],
            thumbnailPhotoId: UUID(),
            createdAt: date("2024-02-01")
        )
        let wrongTag = makeRecipe(title: "Chicken Soup", tags: ["Lunch"], createdAt: date("2024-02-01"))
        let noPhoto = makeRecipe(title: "Chicken Soup", tags: ["Dinner", "Quick"], createdAt: date("2024-02-01"))
        let tooEarly = makeRecipe(
            title: "Chicken Soup",
            tags: ["Dinner", "Quick"],
            thumbnailPhotoId: UUID(),
            createdAt: date("2023-12-31")
        )

        let result = RecipeSummaryCacheSupport.filteredAndSorted(
            [wrongTag, matching, noPhoto, tooEarly],
            filterState: RecipeListFilterState(
                searchText: "chicken dinner",
                selectedTags: ["Dinner", "Quick"],
                photoFilter: .hasPhotos,
                createdAfter: "2024-01-01"
            ),
            sortOrder: .newest
        )

        XCTAssertEqual(result.map(\.id), [matching.id])
    }

    func testSortsRatingsWithUnratedLast() {
        let unrated = makeRecipe(title: "Unrated", rating: nil)
        let five = makeRecipe(title: "Five", rating: 5)
        let three = makeRecipe(title: "Three", rating: 3)

        let result = RecipeSummaryCacheSupport.filteredAndSorted(
            [unrated, three, five],
            filterState: RecipeListFilterState(),
            sortOrder: .rating
        )

        XCTAssertEqual(result.map(\.id), [five.id, three.id, unrated.id])
    }

    private struct CreatedDateFilterCase: Decodable {
        let name: String
        let createdAfter: String?
        let createdBefore: String?
        let createdAt: String
        let matches: Bool
    }

    func testCreatedDateFilterMatchesSharedVectors() throws {
        // Pin a non-UTC device timezone so a pass proves the filter uses UTC
        // days rather than the device calendar.
        let originalTimeZone = NSTimeZone.default
        NSTimeZone.default = TimeZone(identifier: "America/Los_Angeles")!
        defer { NSTimeZone.default = originalTimeZone }

        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "created-date-filter", withExtension: "json")
        )
        let cases = try JSONDecoder().decode([CreatedDateFilterCase].self, from: Data(contentsOf: url))
        // The formatter the app uses to decode createdAt off the wire, so the
        // vectors pin production parse semantics.
        let timestampFormatter = OpenISO8601DateFormatter()

        for testCase in cases {
            let createdAt = try XCTUnwrap(
                timestampFormatter.date(from: testCase.createdAt),
                "unparseable timestamp: \(testCase.createdAt)"
            )
            let recipe = makeRecipe(title: "Boundary", createdAt: createdAt)

            let result = RecipeSummaryCacheSupport.filteredAndSorted(
                [recipe],
                filterState: RecipeListFilterState(
                    createdAfter: testCase.createdAfter ?? "",
                    createdBefore: testCase.createdBefore ?? ""
                ),
                sortOrder: .newest
            )

            XCTAssertEqual(!result.isEmpty, testCase.matches, "case: \(testCase.name)")
        }
    }

    func testSourceFilterAndRandomSortUseNetwork() {
        XCTAssertFalse(
            RecipeSummaryCacheSupport.canServeFromCache(
                filterState: RecipeListFilterState(searchText: "hidden ingredient"),
                sortOrder: .newest
            )
        )
        XCTAssertFalse(
            RecipeSummaryCacheSupport.canServeFromCache(
                filterState: RecipeListFilterState(source: "NYT"),
                sortOrder: .newest
            )
        )
        XCTAssertFalse(
            RecipeSummaryCacheSupport.canServeFromCache(
                filterState: RecipeListFilterState(
                    photoSizeThreshold: NumericThreshold(comparison: .lessThan, value: 100_000)
                ),
                sortOrder: .newest
            )
        )
        XCTAssertFalse(
            RecipeSummaryCacheSupport.canServeFromCache(
                filterState: RecipeListFilterState(),
                sortOrder: .random
            )
        )
        XCTAssertTrue(
            RecipeSummaryCacheSupport.canServeFromCache(
                filterState: RecipeListFilterState(selectedTags: ["Dinner"]),
                sortOrder: .title
            )
        )
    }

    private func makeRecipe(
        title: String,
        description: String? = nil,
        tags: [String] = [],
        thumbnailPhotoId: UUID? = nil,
        rating: Int? = nil,
        createdAt: Date = Date(timeIntervalSince1970: 100),
        updatedAt: Date = Date(timeIntervalSince1970: 200)
    ) -> RecipeSummary {
        RecipeSummary(
            createdAt: createdAt,
            description: description,
            id: UUID(),
            rating: rating,
            tags: tags,
            thumbnailPhotoId: thumbnailPhotoId,
            title: title,
            updatedAt: updatedAt
        )
    }

    private func date(_ value: String) -> Date {
        RecipeListFilterSupport.date(from: value)!
    }

}
