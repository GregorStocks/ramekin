import XCTest
@testable import Ramekin

final class RecipeScaleSupportTests: XCTestCase {
    struct ScaleAmountVector: Decodable {
        let name: String
        let amount: String
        let factor: Double
        let expected: String
    }

    func testScaleAmountMatchesSharedVectors() throws {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "scale-amount", withExtension: "json")
        )
        let vectors = try JSONDecoder().decode(
            [ScaleAmountVector].self,
            from: Data(contentsOf: url)
        )

        for vector in vectors {
            XCTAssertEqual(
                RecipeScaleSupport.scaleAmount(vector.amount, by: vector.factor),
                vector.expected,
                vector.name
            )
        }
    }

    func testParseDecimalAcceptsCommaDecimalInput() {
        XCTAssertEqual(RecipeScaleSupport.parseDecimal("1.5"), 1.5)
        XCTAssertEqual(RecipeScaleSupport.parseDecimal("1,5"), 1.5)
        XCTAssertNil(RecipeScaleSupport.parseDecimal("1,2,3"))
    }

    func testIngredientFormattingScalesPrimaryAndAlternativeMeasurements() {
        let ingredient = Ingredient(
            item: "butter",
            measurements: [
                Measurement(amount: "1/2", unit: "cup"),
                Measurement(amount: "113", unit: "g")
            ],
            note: "softened"
        )

        XCTAssertEqual(
            ingredient.formatted(scale: 2, includeAlternatives: true, includeNote: true),
            "1 cup (226 g) butter (softened)"
        )
    }

    func testShoppingListIngredientsUseScaledAmountsInRecipeOrder() {
        let recipe = RecipeResponse(
            cookTime: nil,
            createdAt: Date(),
            description: nil,
            difficulty: nil,
            id: UUID(),
            ingredients: [
                Ingredient(
                    item: "flour",
                    measurements: [Measurement(amount: "1/2", unit: "cup")]
                ),
                Ingredient(
                    item: "salt",
                    measurements: [Measurement(amount: "1", unit: "tsp")]
                ),
                Ingredient(
                    item: "pepper",
                    measurements: []
                )
            ],
            instructions: "Mix.",
            notes: nil,
            nutritionalInfo: nil,
            photoIds: [],
            prepTime: nil,
            rating: nil,
            servings: "2",
            sourceName: nil,
            sourceUrl: nil,
            tags: [],
            title: "Test Recipe",
            totalTime: nil,
            updatedAt: Date(),
            versionId: UUID(),
            versionSource: "manual"
        )

        let ingredients = AddToShoppingListSheetSupport.ingredientsForShoppingList(
            recipe: recipe,
            selectedIngredients: [1, 0, 2],
            scale: 2
        )

        XCTAssertEqual(ingredients.map(\.name), ["flour", "salt", "pepper"])
        XCTAssertEqual(ingredients[0].amount, "1 cup")
        XCTAssertEqual(ingredients[1].amount, "2 tsp")
        XCTAssertNil(ingredients[2].amount)
    }
}
