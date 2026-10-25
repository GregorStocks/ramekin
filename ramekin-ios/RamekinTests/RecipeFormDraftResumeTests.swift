import XCTest
@testable import Ramekin

/// Resuming a recipe form the app was terminated on.
@MainActor
final class RecipeFormDraftResumeTests: XCTestCase {
    func testResumingNewRecipeRestoresTextAndReview() {
        let prepared = PrepareTextRecipeResponse(
            content: RecipeContent(ingredients: [], instructions: "Mix.", title: "Pancakes"),
            derivedMeasurements: [],
            rawIngredients: "1 cup flour",
            warnings: []
        )
        var form = RecipeFormData(content: prepared.content)
        form.title = "Better pancakes"
        let resumed = RecipeFormDraft(
            mode: .create,
            form: form,
            recipeText: "Pancakes",
            rawIngredients: "2 cups flour",
            prepared: prepared
        )

        let model = RecipeFormViewModel(mode: .create, resuming: resumed, api: unexpectedCallsClient())

        XCTAssertEqual(model.formData.title, "Better pancakes")
        XCTAssertEqual(model.recipeText, "Pancakes")
        XCTAssertEqual(model.rawIngredients, "2 cups flour")
        XCTAssertEqual(model.draft, prepared)
        XCTAssertTrue(model.hasUnsavedChanges)
        XCTAssertEqual(model.resumableDraft, resumed)
    }

    func testFreshFormHasNoDraftToKeep() {
        let model = RecipeFormViewModel(mode: .create, api: unexpectedCallsClient())

        XCTAssertNil(model.resumableDraft)
        model.recipeText = "Soup"
        XCTAssertEqual(model.resumableDraft?.recipeText, "Soup")
    }

    func testResumingEditOfUnchangedRecipeKeepsEditsWithoutWarning() async {
        let recipe = makeRecipe(versionId: UUID())
        var form = RecipeFormData(recipe: recipe)
        form.title = "Edited"
        let model = RecipeFormViewModel(
            mode: .edit(recipeId: recipe.id),
            resuming: RecipeFormDraft(mode: .edit(recipeId: recipe.id), form: form),
            api: unexpectedCallsClient(getRecipe: { _ in recipe })
        )

        await model.loadRecipe(id: recipe.id)

        XCTAssertEqual(model.formData.title, "Edited")
        XCTAssertFalse(model.resumedOverNewerVersion)
        XCTAssertEqual(model.formData.expectedVersionId, recipe.versionId)
        XCTAssertTrue(model.hasUnsavedChanges)
    }

    func testResumingEditOfChangedRecipeWarnsAndSavesOverNewVersion() async {
        let recipe = makeRecipe(versionId: UUID())
        var form = RecipeFormData(recipe: recipe)
        form.title = "Edited"
        form.expectedVersionId = UUID()
        let model = RecipeFormViewModel(
            mode: .edit(recipeId: recipe.id),
            resuming: RecipeFormDraft(mode: .edit(recipeId: recipe.id), form: form),
            api: unexpectedCallsClient(getRecipe: { _ in recipe })
        )

        await model.loadRecipe(id: recipe.id)

        XCTAssertEqual(model.formData.title, "Edited")
        XCTAssertTrue(model.resumedOverNewerVersion)
        XCTAssertEqual(model.formData.expectedVersionId, recipe.versionId)
    }

    private func unexpectedCallsClient(
        getRecipe: @escaping (UUID) async throws -> RecipeResponse = { _ in throw TestError.unexpectedCall }
    ) -> RecipeFormViewAPIClient {
        RecipeFormViewAPIClient(
            createRecipe: { _ in throw TestError.unexpectedCall },
            updateRecipe: { _, _ in throw TestError.unexpectedCall },
            getRecipe: getRecipe,
            syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
            uploadPhoto: { _ in throw TestError.unexpectedCall },
            prepareTextRecipe: { _ in throw TestError.unexpectedCall }
        )
    }

    private func makeRecipe(versionId: UUID) -> RecipeResponse {
        RecipeResponse(
            cookTime: nil,
            createdAt: Date(timeIntervalSince1970: 100),
            derivedMeasurements: [],
            description: nil,
            difficulty: nil,
            id: UUID(),
            ingredients: [],
            instructions: "Simmer",
            normalizeOps: [],
            notes: nil,
            nutritionalInfo: nil,
            photoIds: [],
            prepTime: nil,
            rating: nil,
            servings: nil,
            sourceName: nil,
            sourceUrl: nil,
            tags: [],
            title: "Soup",
            totalTime: nil,
            updatedAt: Date(timeIntervalSince1970: 200),
            versionId: versionId,
            versionSource: "user"
        )
    }

    private enum TestError: Error {
        case unexpectedCall
    }
}
