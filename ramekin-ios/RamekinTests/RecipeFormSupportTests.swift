import XCTest
@testable import Ramekin

final class RecipeFormSupportTests: XCTestCase {
    func testRecipeFormDataPopulatesFromRecipeResponse() {
        let photoId = UUID()
        let recipe = RecipeResponse(
            cookTime: "20 min",
            createdAt: Date(timeIntervalSince1970: 100),
            description: "Tangy and rich",
            difficulty: "Easy",
            id: UUID(),
            ingredients: [
                Ingredient(
                    item: "Flour",
                    measurements: [Measurement(amount: "2", unit: "cups")],
                    note: "sifted",
                    section: "Cake"
                )
            ],
            instructions: "Mix and bake",
            notes: "Serve warm",
            nutritionalInfo: "200 cal",
            photoIds: [photoId],
            prepTime: "10 min",
            rating: 4,
            servings: "6",
            sourceName: "Grandma",
            sourceUrl: "https://example.com/cake",
            tags: ["dessert", "cake"],
            title: "Cake",
            totalTime: "30 min",
            updatedAt: Date(timeIntervalSince1970: 200),
            versionId: UUID(),
            versionSource: "user"
        )

        let formData = RecipeFormData(recipe: recipe)

        XCTAssertEqual(formData.title, "Cake")
        XCTAssertEqual(formData.recipeDescription, "Tangy and rich")
        XCTAssertEqual(formData.instructions, "Mix and bake")
        XCTAssertEqual(formData.servings, "6")
        XCTAssertEqual(formData.prepTime, "10 min")
        XCTAssertEqual(formData.cookTime, "20 min")
        XCTAssertEqual(formData.totalTime, "30 min")
        XCTAssertEqual(formData.difficulty, "Easy")
        XCTAssertEqual(formData.rating, 4)
        XCTAssertEqual(formData.sourceUrl, "https://example.com/cake")
        XCTAssertEqual(formData.sourceName, "Grandma")
        XCTAssertEqual(formData.tags, ["dessert", "cake"])
        XCTAssertEqual(formData.notes, "Serve warm")
        XCTAssertEqual(formData.nutritionalInfo, "200 cal")
        XCTAssertEqual(formData.photoIds, [photoId])
        XCTAssertEqual(formData.ingredients.count, 1)
        XCTAssertEqual(formData.ingredients[0].item, "Flour")
        XCTAssertEqual(formData.ingredients[0].section, "Cake")
        XCTAssertEqual(formData.ingredients[0].measurements[0].amount, "2")
        XCTAssertEqual(formData.ingredients[0].measurements[0].unit, "cups")
    }

    func testRecipeFormDataUsesEmptyIngredientWhenRecipeHasNone() {
        let recipe = RecipeResponse(
            cookTime: nil,
            createdAt: Date(timeIntervalSince1970: 100),
            description: nil,
            difficulty: nil,
            id: UUID(),
            ingredients: [],
            instructions: "Mix",
            notes: nil,
            nutritionalInfo: nil,
            photoIds: [],
            prepTime: nil,
            rating: nil,
            servings: nil,
            sourceName: nil,
            sourceUrl: nil,
            tags: [],
            title: "Plain",
            totalTime: nil,
            updatedAt: Date(timeIntervalSince1970: 200),
            versionId: UUID(),
            versionSource: "user"
        )

        let formData = RecipeFormData(recipe: recipe)

        XCTAssertEqual(formData.ingredients.count, 1)
        XCTAssertEqual(formData.ingredients[0].item, "")
        XCTAssertEqual(formData.ingredients[0].note, "")
        XCTAssertEqual(formData.ingredients[0].section, "")
        XCTAssertEqual(formData.ingredients[0].measurements.count, 1)
        XCTAssertEqual(formData.ingredients[0].measurements[0].amount, "")
        XCTAssertEqual(formData.ingredients[0].measurements[0].unit, "")
    }

    func testCreateRequestFiltersBlankIngredientAndOmitsEmptyOptionalFields() {
        let photoId = UUID()
        let formData = RecipeFormData(
            title: "Cake",
            recipeDescription: "",
            instructions: "Mix and bake",
            servings: "",
            prepTime: "10 min",
            cookTime: "",
            totalTime: "",
            difficulty: "",
            rating: 5,
            sourceUrl: "",
            sourceName: "Bakery",
            tags: [],
            notes: "",
            nutritionalInfo: "200 cal",
            ingredients: [
                EditableIngredient(
                    item: "Flour",
                    measurements: [EditableMeasurement(amount: "2", unit: "cups")],
                    note: "",
                    section: "Cake"
                ),
                EditableIngredient.empty()
            ],
            photoIds: [photoId]
        )

        let request = formData.makeCreateRequest()

        XCTAssertEqual(request.title, "Cake")
        XCTAssertEqual(request.instructions, "Mix and bake")
        XCTAssertEqual(request.prepTime, "10 min")
        XCTAssertEqual(request.rating, 5)
        XCTAssertEqual(request.sourceName, "Bakery")
        XCTAssertEqual(request.nutritionalInfo, "200 cal")
        XCTAssertEqual(request.photoIds, [photoId])
        XCTAssertNil(request.description)
        XCTAssertNil(request.servings)
        XCTAssertNil(request.cookTime)
        XCTAssertNil(request.sourceUrl)
        XCTAssertNil(request.tags)
        XCTAssertEqual(request.ingredients.count, 1)
        XCTAssertEqual(request.ingredients[0].item, "Flour")
        XCTAssertEqual(request.ingredients[0].section, "Cake")
    }

    func testUpdateRequestPreservesExplicitEmptyTagsAndPhotoIds() {
        let expectedVersionId = UUID()
        let formData = RecipeFormData(
            title: "Soup",
            recipeDescription: "Brothy",
            instructions: "Simmer",
            servings: "4",
            prepTime: "",
            cookTime: "30 min",
            totalTime: "45 min",
            difficulty: "Easy",
            rating: nil,
            sourceUrl: "https://example.com/soup",
            sourceName: "",
            tags: [],
            notes: "Salt to taste",
            nutritionalInfo: "",
            ingredients: [
                EditableIngredient(
                    item: "Water",
                    measurements: [EditableMeasurement(amount: "4", unit: "cups")],
                    note: "",
                    section: ""
                )
            ],
            photoIds: [],
            expectedVersionId: expectedVersionId
        )

        let request = formData.makeUpdateRequest()

        XCTAssertEqual(request.title, "Soup")
        XCTAssertEqual(request.description, "Brothy")
        XCTAssertEqual(request.instructions, "Simmer")
        XCTAssertEqual(request.servings, "4")
        XCTAssertEqual(request.cookTime, "30 min")
        XCTAssertEqual(request.totalTime, "45 min")
        XCTAssertEqual(request.difficulty, "Easy")
        XCTAssertEqual(request.sourceUrl, "https://example.com/soup")
        XCTAssertEqual(request.notes, "Salt to taste")
        XCTAssertEqual(request.expectedVersionId, expectedVersionId)
        XCTAssertEqual(request.tags, [])
        XCTAssertEqual(request.photoIds, [])
        XCTAssertNil(request.prepTime)
        XCTAssertNil(request.sourceName)
        XCTAssertNil(request.nutritionalInfo)
    }
}
