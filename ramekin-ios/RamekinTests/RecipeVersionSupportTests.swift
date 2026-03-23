import XCTest
@testable import Ramekin

final class RecipeVersionSupportTests: XCTestCase {
    func testSourceLabelMapsKnownValues() {
        XCTAssertEqual(RecipeVersionSupport.sourceLabel(for: "user"), "User Edit")
        XCTAssertEqual(RecipeVersionSupport.sourceLabel(for: "scrape"), "Imported")
        XCTAssertEqual(RecipeVersionSupport.sourceLabel(for: "enrichment"), "AI Enriched")
        XCTAssertEqual(RecipeVersionSupport.sourceLabel(for: "custom"), "custom")
    }

    func testIsViewingHistoricalVersionRequiresDifferentDisplayedVersion() {
        let currentVersionId = UUID()

        XCTAssertFalse(
            RecipeVersionSupport.isViewingHistoricalVersion(
                displayedVersionId: nil,
                currentVersionId: currentVersionId
            )
        )
        XCTAssertFalse(
            RecipeVersionSupport.isViewingHistoricalVersion(
                displayedVersionId: currentVersionId,
                currentVersionId: currentVersionId
            )
        )
        XCTAssertTrue(
            RecipeVersionSupport.isViewingHistoricalVersion(
                displayedVersionId: UUID(),
                currentVersionId: currentVersionId
            )
        )
    }

    func testToggleCompareSelectionKeepsOnlyTwoMostRecentSelections() {
        let first = UUID()
        let second = UUID()
        let third = UUID()

        XCTAssertEqual(
            RecipeVersionSupport.toggleCompareSelection([], versionId: first),
            [first]
        )
        XCTAssertEqual(
            RecipeVersionSupport.toggleCompareSelection([first], versionId: second),
            [first, second]
        )
        XCTAssertEqual(
            RecipeVersionSupport.toggleCompareSelection([first, second], versionId: third),
            [second, third]
        )
        XCTAssertEqual(
            RecipeVersionSupport.toggleCompareSelection([second, third], versionId: second),
            [third]
        )
    }

    func testSortForCompareOrdersOlderVersionFirst() {
        let olderVersion = makeRecipe(updatedAt: Date(timeIntervalSince1970: 100))
        let newerVersion = makeRecipe(updatedAt: Date(timeIntervalSince1970: 200))

        let sorted = RecipeVersionSupport.sortForCompare(newerVersion, olderVersion)

        XCTAssertEqual(sorted.older.updatedAt, olderVersion.updatedAt)
        XCTAssertEqual(sorted.newer.updatedAt, newerVersion.updatedAt)
    }

    func testUpdateRequestCopiesRecipeFieldsForRevert() {
        let recipe = makeRecipe(
            updatedAt: Date(timeIntervalSince1970: 300),
            photoIds: [UUID(), UUID()]
        )

        let request = RecipeVersionSupport.updateRequest(for: recipe)

        XCTAssertEqual(request.title, recipe.title)
        XCTAssertEqual(request.description, recipe.description)
        XCTAssertEqual(request.instructions, recipe.instructions)
        XCTAssertEqual(request.ingredients, recipe.ingredients)
        XCTAssertEqual(request.tags, recipe.tags)
        XCTAssertEqual(request.photoIds, recipe.photoIds)
        XCTAssertEqual(request.rating, recipe.rating)
        XCTAssertEqual(request.sourceUrl, recipe.sourceUrl)
    }

    func testFormatIngredientsUsesPrimaryMeasurementAndItem() {
        let ingredients = [
            Ingredient(
                item: "flour",
                measurements: [
                    Measurement(amount: "1", unit: "cup"),
                    Measurement(amount: "120", unit: "g")
                ]
            ),
            Ingredient(
                item: "salt",
                measurements: []
            )
        ]

        XCTAssertEqual(
            RecipeVersionSupport.formatIngredients(ingredients),
            "1 cup flour\nsalt"
        )
    }

    private func makeRecipe(
        updatedAt: Date,
        photoIds: [UUID] = [UUID()]
    ) -> RecipeResponse {
        RecipeResponse(
            cookTime: "20 min",
            createdAt: Date(timeIntervalSince1970: 50),
            description: "Test description",
            difficulty: "Easy",
            id: UUID(),
            ingredients: [
                Ingredient(
                    item: "flour",
                    measurements: [Measurement(amount: "1", unit: "cup")],
                    note: "sifted",
                    raw: "1 cup flour, sifted",
                    section: "Batter"
                )
            ],
            instructions: "Mix everything.",
            notes: "Test notes",
            nutritionalInfo: "200 calories",
            photoIds: photoIds,
            prepTime: "10 min",
            rating: 4,
            servings: "2",
            sourceName: "Example Site",
            sourceUrl: "https://example.com/recipe",
            tags: ["dessert", "quick"],
            title: "Test Recipe",
            totalTime: "30 min",
            updatedAt: updatedAt,
            versionId: UUID(),
            versionSource: "user"
        )
    }
}
