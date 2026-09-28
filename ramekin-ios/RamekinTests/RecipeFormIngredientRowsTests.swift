import XCTest
@testable import Ramekin

@MainActor
final class RecipeFormIngredientRowsTests: XCTestCase {
    private func ingredient(_ item: String) -> IngredientEditorRow {
        .ingredient(EditableIngredient(item: item, measurements: [EditableMeasurement()], note: ""))
    }

    private func sections(_ model: RecipeFormViewModel) -> [String?] {
        ingredients(from: model.formData.ingredientRows).map(\.section)
    }

    private func items(_ model: RecipeFormViewModel) -> [String] {
        ingredients(from: model.formData.ingredientRows).map(\.item)
    }

    func testMovingIngredientPastHeadingChangesItsSection() {
        let model = RecipeFormViewModel(mode: .create)
        model.formData.ingredientRows = [
            ingredient("salt"),
            .newSection("Batter"),
            ingredient("flour")
        ]

        // Same offsets List.onMove reports when dragging salt below flour.
        model.moveIngredientRows(from: IndexSet(integer: 0), to: 3)

        XCTAssertEqual(items(model), ["flour", "salt"])
        XCTAssertEqual(sections(model), ["Batter", "Batter"])
    }

    func testDeletingHeadingKeepsIngredientsInSectionAbove() {
        let model = RecipeFormViewModel(mode: .create)
        let glaze = IngredientEditorRow.newSection("Glaze")
        model.formData.ingredientRows = [
            .newSection("Batter"),
            ingredient("flour"),
            glaze,
            ingredient("sugar")
        ]

        model.removeIngredientRow(id: glaze.id)

        XCTAssertEqual(items(model), ["flour", "sugar"])
        XCTAssertEqual(sections(model), ["Batter", "Batter"])
    }

    func testAddIngredientToSectionInsertsBeforeNextHeading() {
        let model = RecipeFormViewModel(mode: .create)
        let batter = IngredientEditorRow.newSection("Batter")
        model.formData.ingredientRows = [
            batter,
            ingredient("flour"),
            .newSection("Glaze"),
            ingredient("sugar")
        ]

        model.addIngredient(toSectionWithId: batter.id)

        XCTAssertEqual(model.formData.ingredientRows.count, 5)
        XCTAssertFalse(model.formData.ingredientRows[2].isSection)
        XCTAssertEqual(sections(model), ["Batter", "Batter", "Glaze"])
    }

    func testAddSectionAppendsFocusableHeadingWithEmptyIngredient() {
        let model = RecipeFormViewModel(mode: .create)
        model.formData.ingredientRows = [ingredient("salt")]

        let headingId = model.addSection()

        XCTAssertEqual(model.formData.ingredientRows.count, 3)
        XCTAssertEqual(model.formData.ingredientRows[1].id, headingId)
        XCTAssertTrue(model.formData.ingredientRows[1].isSection)
        XCTAssertFalse(model.formData.ingredientRows[2].isSection)
    }

    func testRenamingSectionThroughBindingUpdatesEveryIngredientBelowIt() {
        let model = RecipeFormViewModel(mode: .create)
        let batter = IngredientEditorRow.newSection("Batter")
        model.formData.ingredientRows = [batter, ingredient("flour"), ingredient("milk")]

        model.sectionNameBinding(id: batter.id, rendered: "Batter").wrappedValue = "Cake"

        XCTAssertEqual(sections(model), ["Cake", "Cake"])
    }

    func testBindingToRemovedRowReturnsRenderedValueAndIgnoresWrites() {
        let model = RecipeFormViewModel(mode: .create)
        let batter = IngredientEditorRow.newSection("Batter")
        model.formData.ingredientRows = [batter, ingredient("flour")]
        let binding = model.sectionNameBinding(id: batter.id, rendered: "Batter")

        model.removeIngredientRow(id: batter.id)

        XCTAssertEqual(binding.wrappedValue, "Batter")
        binding.wrappedValue = "Cake"
        XCTAssertEqual(model.formData.ingredientRows.count, 1)
        XCTAssertEqual(sections(model), [nil])
    }
}
