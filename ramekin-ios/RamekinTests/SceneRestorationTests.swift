import XCTest
@testable import Ramekin

final class SceneRestorationTests: XCTestCase {
    private let first = UUID()
    private let second = UUID()

    func testPathRoundTrips() {
        let path: [NavigationDestination] = [.settings, .recipe(first), .recipe(second)]

        XCTAssertEqual(SceneRestoration.decodePath(SceneRestoration.encodePath(path)), path)
    }

    func testUnreadablePathRestoresAsEmpty() {
        XCTAssertEqual(SceneRestoration.decodePath(Data()), [])
        XCTAssertEqual(SceneRestoration.decodePath(Data("not json".utf8)), [])
    }

    func testPrunedPathStopsAtFirstMissingRecipe() {
        let path: [NavigationDestination] = [.settings, .recipe(first), .recipe(second)]

        XCTAssertEqual(
            SceneRestoration.prunedPath(path) { $0 != self.first },
            [.settings]
        )
        XCTAssertEqual(
            SceneRestoration.prunedPath(path) { $0 != self.second },
            [.settings, .recipe(first)]
        )
        XCTAssertEqual(SceneRestoration.prunedPath(path) { _ in true }, path)
    }

    func testRecipeIdsInPath() {
        let path: [NavigationDestination] = [.recipe(first), .settings, .recipe(second)]

        XCTAssertEqual(SceneRestoration.recipeIds(in: path), [first, second])
    }

    func testPlacesRoundTrip() {
        let places = [
            first: RecipePlace(scale: 2, customInput: "", scrollAnchor: "step-3"),
            second: RecipePlace(scale: 1.5, customInput: "1.5")
        ]

        XCTAssertEqual(SceneRestoration.decodePlaces(SceneRestoration.encodePlaces(places)), places)
        XCTAssertEqual(SceneRestoration.decodePlaces(Data("garbage".utf8)), [:])
    }

    func testPrunedPlacesKeepsOnlyOpenRecipes() {
        let places = [
            first: RecipePlace(scale: 2),
            second: RecipePlace(scale: 3)
        ]

        XCTAssertEqual(
            SceneRestoration.prunedPlaces(places, keeping: [second]),
            [second: RecipePlace(scale: 3)]
        )
    }

    func testPlaceMemoryUpdateKeepsOtherFields() {
        var stored: [UUID: RecipePlace] = [first: RecipePlace(scale: 2, customInput: "2")]
        let memory = RecipePlaceMemory(stored: { stored[$0] }, remember: { stored[$0] = $1 })

        memory.update(first) { $0.scrollAnchor = "ingredient-4" }
        memory.update(second) { $0.scrollAnchor = "step-0" }

        XCTAssertEqual(stored[first], RecipePlace(scale: 2, customInput: "2", scrollAnchor: "ingredient-4"))
        XCTAssertEqual(stored[second], RecipePlace(scrollAnchor: "step-0"))
    }

    func testTopAnchorIsLowestAnchorAtOrAboveViewportTop() {
        XCTAssertNil(SceneRestoration.topAnchor([:]))
        XCTAssertNil(
            SceneRestoration.topAnchor(["ingredient-0": 120, "step-0": 600]),
            "Nothing scrolled past yet means the top of the recipe"
        )
        XCTAssertEqual(
            SceneRestoration.topAnchor(["ingredient-0": -300, "ingredient-1": -20, "step-0": 40]),
            "ingredient-1",
            "A partly scrolled-off row is restored so none of it is skipped"
        )
        XCTAssertEqual(
            SceneRestoration.topAnchor(["step-2": -40, "step-3": 0.5, "step-4": 30]),
            "step-3",
            "A row restored to the top stays the anchor despite rounding"
        )
    }

    func testTopVisibleItemFollowsListOrder() {
        let (milk, eggs, bread) = (UUID(), UUID(), UUID())
        XCTAssertNil(SceneRestoration.topVisibleItem(in: [milk, eggs], visible: []))
        XCTAssertEqual(
            SceneRestoration.topVisibleItem(in: [milk, eggs, bread], visible: [bread, eggs]),
            eggs,
            "The first visible row in list order, not set order"
        )
        XCTAssertNil(
            SceneRestoration.topVisibleItem(in: [milk], visible: [eggs]),
            "A row no longer listed isn't restored"
        )
    }

    func testDraftsRoundTrip() throws {
        var createForm = RecipeFormData()
        createForm.title = "Pancakes"
        var editForm = RecipeFormData()
        editForm.title = "Soup"
        editForm.expectedVersionId = second
        editForm.ingredientRows = [
            .ingredient(EditableIngredient(item: "salt", measurements: [EditableMeasurement()], note: "")),
            .newSection("Broth"),
            .ingredient(EditableIngredient(
                item: "stock",
                measurements: [EditableMeasurement(amount: "2", unit: "cups")],
                note: "low sodium"
            ))
        ]
        let drafts = [
            RecipeFormDraft(
                mode: .create,
                form: createForm,
                recipeText: "Pancakes\n1 cup flour"
            ),
            RecipeFormDraft(mode: .edit(recipeId: first), form: editForm)
        ]

        let decoded = SceneRestoration.decodeDrafts(SceneRestoration.encodeDrafts(drafts))

        XCTAssertEqual(decoded.count, 2)
        XCTAssertEqual(decoded[0].mode, .create)
        XCTAssertEqual(decoded[0].form.title, "Pancakes")
        XCTAssertEqual(decoded[0].recipeText, "Pancakes\n1 cup flour")
        XCTAssertEqual(decoded[1].mode, .edit(recipeId: first))
        XCTAssertEqual(decoded[1].form.expectedVersionId, second)
        XCTAssertEqual(
            ingredients(from: decoded[1].form.ingredientRows),
            ingredients(from: editForm.ingredientRows),
            "Ingredients and sections survive, though rows get fresh ids"
        )
        XCTAssertEqual(SceneRestoration.decodeDrafts(Data("garbage".utf8)), [])
    }

    func testReplacingDraftReplacesOnlyThatForm() {
        let create = RecipeFormDraft(mode: .create, form: RecipeFormData(), recipeText: "Soup")
        let edit = RecipeFormDraft(mode: .edit(recipeId: first), form: RecipeFormData())
        let newerCreate = RecipeFormDraft(mode: .create, form: RecipeFormData(), recipeText: "Stew")

        let replaced = SceneRestoration.replacingDraft(in: [create, edit], for: .create, with: newerCreate)
        XCTAssertEqual(replaced.map(\.recipeText), ["", "Stew"])
        XCTAssertEqual(
            SceneRestoration.replacingDraft(in: [create, edit], for: .edit(recipeId: first), with: nil)
                .map(\.mode),
            [.create]
        )
    }

    func testPrunedDraftsKeepsNewRecipeAndOpenRecipesEdits() {
        let create = RecipeFormDraft(mode: .create, form: RecipeFormData())
        let firstEdit = RecipeFormDraft(mode: .edit(recipeId: first), form: RecipeFormData())
        let secondEdit = RecipeFormDraft(mode: .edit(recipeId: second), form: RecipeFormData())

        XCTAssertEqual(
            SceneRestoration.prunedDrafts([create, firstEdit, secondEdit], keeping: [second]).map(\.mode),
            [.create, .edit(recipeId: second)]
        )
    }
}
