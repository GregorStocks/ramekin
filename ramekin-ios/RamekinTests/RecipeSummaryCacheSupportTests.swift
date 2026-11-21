import XCTest
@testable import Ramekin

final class RecipeSummaryCacheSupportTests: XCTestCase {
    func testFiltersBySearchTagsPhotosAndCreatedDate() {
        let matching = makeDocument(
            title: "Chicken Soup",
            description: "Weeknight dinner",
            tags: ["Dinner", "Quick"],
            thumbnailPhotoId: UUID(),
            createdAt: date("2024-02-01")
        )
        let wrongTag = makeDocument(title: "Chicken Soup", description: "Weeknight dinner", tags: ["Lunch"], createdAt: date("2024-02-01"))
        let noPhoto = makeDocument(title: "Chicken Soup", description: "Weeknight dinner", tags: ["Dinner", "Quick"], createdAt: date("2024-02-01"))
        let tooEarly = makeDocument(
            title: "Chicken Soup",
            description: "Weeknight dinner",
            tags: ["Dinner", "Quick"],
            thumbnailPhotoId: UUID(),
            createdAt: date("2023-12-31")
        )

        let result = RecipeSummaryCacheSupport.visibleRecipes(
            documents: [wrongTag, matching, noPhoto, tooEarly],
            filterState: RecipeListFilterState(
                searchText: "chicken dinner",
                selectedTags: ["Dinner", "Quick"],
                photoFilter: .hasPhotos,
                createdAfter: "2024-01-01"
            ),
            sortOrder: .newest
        )

        XCTAssertEqual(result.map(\.id), [matching.summary.id])
    }

    func testBareTextMatchesBodyFieldsButNotTags() {
        let viaInstructions = makeDocument(title: "Plain", instructions: "Add the garlic.")
        let viaMatchText = makeDocument(title: "Boring", ingredientMatchText: "[{\"item\": \"garlic\"}]")
        let viaTagOnly = makeDocument(title: "Tagged", tags: ["garlic"])

        let result = RecipeSummaryCacheSupport.visibleRecipes(
            documents: [viaInstructions, viaMatchText, viaTagOnly],
            filterState: RecipeListFilterState(searchText: "garlic"),
            sortOrder: .newest
        )

        XCTAssertEqual(
            Set(result.map(\.id)),
            [viaInstructions.summary.id, viaMatchText.summary.id]
        )
    }

    func testTagFilterIsCaseInsensitiveButAccentSensitive() {
        let dinner = makeDocument(title: "Stew", tags: ["Dinner"])
        let creme = makeDocument(title: "Custard", tags: ["Crème"])

        let lowercased = RecipeSummaryCacheSupport.visibleRecipes(
            documents: [dinner, creme],
            filterState: RecipeListFilterState(searchText: "tag:dinner"),
            sortOrder: .newest
        )
        XCTAssertEqual(lowercased.map(\.id), [dinner.summary.id])

        let unaccented = RecipeSummaryCacheSupport.visibleRecipes(
            documents: [dinner, creme],
            filterState: RecipeListFilterState(searchText: "tag:creme"),
            sortOrder: .newest
        )
        XCTAssertTrue(unaccented.isEmpty)

        let accented = RecipeSummaryCacheSupport.visibleRecipes(
            documents: [dinner, creme],
            filterState: RecipeListFilterState(searchText: "tag:CRÈME"),
            sortOrder: .newest
        )
        XCTAssertEqual(accented.map(\.id), [creme.summary.id])
    }

    func testSortsRatingsWithUnratedLast() {
        let unrated = makeDocument(title: "Unrated", rating: nil)
        let five = makeDocument(title: "Five", rating: 5)
        let three = makeDocument(title: "Three", rating: 3)

        let result = RecipeSummaryCacheSupport.visibleRecipes(
            documents: [unrated, three, five],
            filterState: RecipeListFilterState(),
            sortOrder: .rating
        )

        XCTAssertEqual(result.map(\.id), [five.summary.id, three.summary.id, unrated.summary.id])
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
            let recipe = makeDocument(title: "Boundary", createdAt: createdAt)

            let result = RecipeSummaryCacheSupport.visibleRecipes(
                documents: [recipe],
                filterState: RecipeListFilterState(
                    createdAfter: testCase.createdAfter ?? "",
                    createdBefore: testCase.createdBefore ?? ""
                ),
                sortOrder: .newest
            )

            XCTAssertEqual(!result.isEmpty, testCase.matches, "case: \(testCase.name)")
        }
    }

    func testSourcePhotoAndCalorieFiltersRunOnSyncedFacts() {
        var bread = makeDocument(title: "Bread", thumbnailPhotoId: UUID())
        bread.sourceName = "Café Délices"
        bread.kcalPerServing = 250
        bread.photos = [SyncPhoto(fileSize: 90_000, height: 600, width: 800)]
        var soup = makeDocument(title: "Soup", thumbnailPhotoId: UUID())
        soup.kcalPerServing = 480
        // A photo whose facts are unknown never satisfies a threshold.
        soup.photos = [SyncPhoto()]
        let plain = makeDocument(title: "Plain")

        func titles(_ filterState: RecipeListFilterState) -> [String] {
            RecipeSummaryCacheSupport.visibleRecipes(
                documents: [bread, soup, plain],
                filterState: filterState,
                sortOrder: .title
            ).map(\.title)
        }

        XCTAssertEqual(titles(RecipeListFilterState(source: "cafe")), ["Bread"])
        XCTAssertEqual(
            titles(RecipeListFilterState(photoSizeThreshold: NumericThreshold(comparison: .lessThan, value: 100_000))),
            ["Bread"]
        )
        XCTAssertEqual(
            titles(RecipeListFilterState(photoDimensionThreshold: NumericThreshold(comparison: .greaterThan, value: 600))),
            []
        )
        XCTAssertEqual(
            titles(RecipeListFilterState(photoDimensionThreshold: NumericThreshold(comparison: .greaterThan, value: 599))),
            ["Bread"]
        )
        XCTAssertEqual(titles(RecipeListFilterState(caloriesMin: "300")), ["Soup"])
        XCTAssertEqual(titles(RecipeListFilterState(caloriesMin: "250", caloriesMax: "480")), ["Bread", "Soup"])
    }

    func testCalorieSortsPutRecipesWithoutAnEstimateLast() {
        var light = makeDocument(title: "Light")
        light.kcalPerServing = 120
        var heavy = makeDocument(title: "Heavy")
        heavy.kcalPerServing = 900
        let unknown = makeDocument(title: "Unknown")

        func titles(_ sortOrder: RecipeSortOrder) -> [String] {
            RecipeSummaryCacheSupport.visibleRecipes(
                documents: [unknown, heavy, light],
                filterState: RecipeListFilterState(),
                sortOrder: sortOrder
            ).map(\.title)
        }

        XCTAssertEqual(titles(.caloriesLow), ["Light", "Heavy", "Unknown"])
        XCTAssertEqual(titles(.caloriesHigh), ["Heavy", "Light", "Unknown"])
    }

    func testRandomSortReturnsEveryMatchInSomeOrder() {
        let documents = (0..<40).map { makeDocument(title: "Recipe \($0)", tags: $0 < 30 ? ["Dinner"] : []) }
        let expected = Set(documents.prefix(30).map(\.summary.id))
        let filterState = RecipeListFilterState(selectedTags: ["Dinner"])

        let orders = (0..<5).map { _ in
            RecipeSummaryCacheSupport.visibleRecipes(documents: documents, filterState: filterState, sortOrder: .random)
                .map(\.id)
        }

        for order in orders {
            XCTAssertEqual(order.count, 30)
            XCTAssertEqual(Set(order), expected)
        }
        // Thirty recipes shuffled five times do not come out the same way.
        XCTAssertGreaterThan(Set(orders).count, 1)
    }

    private func makeDocument(
        title: String,
        description: String? = nil,
        tags: [String] = [],
        thumbnailPhotoId: UUID? = nil,
        rating: Int? = nil,
        createdAt: Date = Date(timeIntervalSince1970: 100),
        updatedAt: Date = Date(timeIntervalSince1970: 200),
        ingredientMatchText: String = "[]",
        instructions: String = "",
        notes: String? = nil
    ) -> CachedRecipeSearchDocument {
        CachedRecipeSearchDocument(
            summary: RecipeSummary(
                createdAt: createdAt,
                description: description,
                id: UUID(),
                rating: rating,
                tags: tags,
                thumbnailPhotoId: thumbnailPhotoId,
                title: title,
                updatedAt: updatedAt
            ),
            ingredients: [],
            ingredientMatchText: ingredientMatchText,
            instructions: instructions,
            notes: notes
        )
    }

    private func date(_ value: String) -> Date {
        RecipeListFilterSupport.date(from: value)!
    }
}
