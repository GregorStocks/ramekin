import XCTest
@testable import Ramekin

final class IngredientSectionGroupingTests: XCTestCase {
    func testGroupConsecutiveItemsBySectionPreservesRuns() {
        let groups = groupConsecutiveItemsBySection(
            [
                TestItem(name: "intro-1", section: nil),
                TestItem(name: "intro-2", section: nil),
                TestItem(name: "batter-1", section: "Batter"),
                TestItem(name: "batter-2", section: "Batter"),
                TestItem(name: "intro-3", section: nil)
            ]
        ) { $0.section }

        XCTAssertEqual(groups.map(\.section), [nil, "Batter", nil])
        XCTAssertEqual(
            groups.map { $0.items.map(\.name) },
            [["intro-1", "intro-2"], ["batter-1", "batter-2"], ["intro-3"]]
        )
    }

    func testEditableIngredientGroupingMapsSectionsToIndices() {
        let groups = groupIngredientsBySection([
            EditableIngredient.empty(section: ""),
            EditableIngredient.empty(section: ""),
            EditableIngredient.empty(section: "Sauce"),
            EditableIngredient.empty(section: "Sauce"),
            EditableIngredient.empty(section: "")
        ])

        XCTAssertEqual(groups.map(\.section), ["", "Sauce", ""])
        XCTAssertEqual(groups.map(\.indices), [[0, 1], [2, 3], [4]])
    }

    func testRecipeDetailGroupingReturnsGroupedIngredients() {
        let view = RecipeDetailView(recipeId: UUID())
        let ingredients = [
            Ingredient(item: "salt", measurements: [], section: nil),
            Ingredient(item: "pepper", measurements: [], section: nil),
            Ingredient(item: "flour", measurements: [], section: "Batter"),
            Ingredient(item: "milk", measurements: [], section: "Batter"),
            Ingredient(item: "oil", measurements: [], section: nil)
        ]

        let groups = view.groupIngredientsBySection(ingredients)

        XCTAssertEqual(groups.map(\.section), [nil, "Batter", nil])
        XCTAssertEqual(
            groups.map { $0.items.map(\.item) },
            [["salt", "pepper"], ["flour", "milk"], ["oil"]]
        )
    }
}

private struct TestItem {
    let name: String
    let section: String?
}
