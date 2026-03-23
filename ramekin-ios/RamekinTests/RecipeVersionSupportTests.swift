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

    func testRevertRecipeRequestEncodesExplicitNulls() throws {
        let recipe = makeRecipe(
            updatedAt: Date(timeIntervalSince1970: 300),
            photoIds: [UUID(), UUID()]
        )

        let clearedRecipe = RecipeResponse(
            cookTime: nil,
            createdAt: recipe.createdAt,
            description: nil,
            difficulty: nil,
            id: recipe.id,
            ingredients: recipe.ingredients,
            instructions: recipe.instructions,
            notes: nil,
            nutritionalInfo: nil,
            photoIds: recipe.photoIds,
            prepTime: nil,
            rating: nil,
            servings: nil,
            sourceName: nil,
            sourceUrl: nil,
            tags: recipe.tags,
            title: recipe.title,
            totalTime: nil,
            updatedAt: recipe.updatedAt,
            versionId: recipe.versionId,
            versionSource: recipe.versionSource
        )
        let request = RevertRecipeRequest(recipe: clearedRecipe)
        let data = try JSONEncoder().encode(request)
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: data) as? [String: Any]
        )

        XCTAssertEqual(json["title"] as? String, clearedRecipe.title)
        XCTAssertEqual(json["instructions"] as? String, clearedRecipe.instructions)
        XCTAssertEqual(json["tags"] as? [String], clearedRecipe.tags)
        XCTAssertTrue(json["description"] is NSNull)
        XCTAssertTrue(json["notes"] is NSNull)
        XCTAssertTrue(json["source_name"] is NSNull)
        XCTAssertTrue(json["source_url"] is NSNull)
        XCTAssertTrue(json["rating"] is NSNull)
    }

    func testFormatIngredientsIncludesAlternateMeasurementsNotesAndSections() {
        let ingredients = [
            Ingredient(
                item: "flour",
                measurements: [
                    Measurement(amount: "1", unit: "cup"),
                    Measurement(amount: "120", unit: "g")
                ],
                note: "sifted",
                section: "Batter"
            ),
            Ingredient(
                item: "salt",
                measurements: [],
                section: "Batter"
            )
        ]

        XCTAssertEqual(
            RecipeVersionSupport.formatIngredients(ingredients),
            "[Batter]\n1 cup (120 g) flour (sifted)\nsalt"
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
