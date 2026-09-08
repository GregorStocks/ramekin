import XCTest
@testable import Ramekin

@MainActor
final class RecipeFormViewModelTests: XCTestCase {
    func testTextReviewCorrectAndSave() async {
        var saved: CreateRecipeRequest?
        let content = RecipeContent(
            ingredients: [Ingredient(item: "flour", measurements: [Measurement(amount: "125", unit: "g")])],
            instructions: "Mix.", title: "Pancakes"
        )
        let model = RecipeFormViewModel(mode: .create, api: RecipeFormViewAPIClient(
            createRecipe: { saved = $0 },
            updateRecipe: { _, _ in throw TestError.unexpectedCall },
            getRecipe: { _ in throw TestError.unexpectedCall },
            listAllTags: { TagsListResponse(tags: []) },
            uploadPhoto: { _ in throw TestError.unexpectedCall },
            prepareTextRecipe: { text in
                XCTAssertEqual(text, "Pasted recipe")
                return PrepareTextRecipeResponse(content: content, rawIngredients: "1 cup flour", warnings: [])
            }
        ))
        model.recipeText = "Pasted recipe"
        XCTAssertFalse(model.canSave)
        await model.prepareRecipe()
        XCTAssertTrue(model.canSave)
        XCTAssertNil(saved)
        model.formData.title = "Corrected pancakes"
        model.rawIngredients = "2 cups flour"
        let didSave = await model.save()
        XCTAssertTrue(didSave)
        XCTAssertEqual(saved?.title, "Corrected pancakes")
        XCTAssertEqual(saved?.rawIngredients, "2 cups flour")
        XCTAssertEqual(model.recipeText, "Pasted recipe")
    }

    func testTextAndReviewEditsSurviveFailures() async {
        let model = RecipeFormViewModel(mode: .create, api: RecipeFormViewAPIClient(
            createRecipe: { _ in throw TestError.unexpectedCall },
            updateRecipe: { _, _ in throw TestError.unexpectedCall },
            getRecipe: { _ in throw TestError.unexpectedCall },
            listAllTags: { TagsListResponse(tags: []) },
            uploadPhoto: { _ in throw TestError.unexpectedCall },
            prepareTextRecipe: { _ in throw TestError.unexpectedCall }
        ))
        model.recipeText = "My original recipe"
        await model.prepareRecipe()
        XCTAssertEqual(model.recipeText, "My original recipe")
        XCTAssertNotNil(model.error)
        XCTAssertFalse(model.isPreparing)
        model.formData.title = "My corrected recipe"
        model.rawIngredients = "3 cups flour"
        let didSave = await model.save()
        XCTAssertFalse(didSave)
        XCTAssertEqual(model.formData.title, "My corrected recipe")
        XCTAssertEqual(model.rawIngredients, "3 cups flour")
        XCTAssertEqual(model.recipeText, "My original recipe")
    }

    func testEditSaveRequiresLoadedRecipeVersion() {
        let viewModel = RecipeFormViewModel(
            mode: .edit(recipeId: UUID()),
            api: RecipeFormViewAPIClient(
                createRecipe: { _ in throw TestError.unexpectedCall },
                updateRecipe: { _, _ in throw TestError.unexpectedCall },
                getRecipe: { _ in throw TestError.unexpectedCall },
                listAllTags: { TagsListResponse(tags: []) },
                uploadPhoto: { _ in throw TestError.unexpectedCall }
            )
        )
        viewModel.formData.title = "Soup"
        viewModel.formData.instructions = "Simmer"

        XCTAssertFalse(viewModel.canSave)

        viewModel.formData.expectedVersionId = UUID()

        XCTAssertTrue(viewModel.canSave)
    }

    func testCreateSaveBuildsRequestAndClearsSavingState() async {
        var capturedRequest: CreateRecipeRequest?
        let viewModel = RecipeFormViewModel(
            mode: .create,
            api: RecipeFormViewAPIClient(
                createRecipe: {
                    capturedRequest = $0
                },
                updateRecipe: { _, _ in XCTFail("Expected create, not update") },
                getRecipe: { _ in throw TestError.unexpectedCall },
                listAllTags: { TagsListResponse(tags: []) },
                uploadPhoto: { _ in UploadPhotoResponse(id: UUID()) }
            )
        )
        viewModel.formData.title = "Cake"
        viewModel.formData.instructions = "Mix and bake"
        viewModel.formData.ingredients = [
            EditableIngredient(
                item: "Flour",
                measurements: [EditableMeasurement(amount: "2", unit: "cups")],
                note: "",
                section: "Batter"
            )
        ]

        let didSave = await viewModel.save()

        XCTAssertTrue(didSave)
        XCTAssertFalse(viewModel.isSaving)
        XCTAssertNil(viewModel.error)
        XCTAssertEqual(capturedRequest?.title, "Cake")
        XCTAssertEqual(capturedRequest?.instructions, "Mix and bake")
        XCTAssertEqual(capturedRequest?.ingredients.count, 1)
        XCTAssertEqual(capturedRequest?.ingredients.first?.section, "Batter")
    }

    func testUpdateSaveUsesExistingRecipeId() async {
        let recipeId = UUID()
        var capturedUpdate: (id: UUID, request: UpdateRecipeRequest)?
        let viewModel = RecipeFormViewModel(
            mode: .edit(recipeId: recipeId),
            api: RecipeFormViewAPIClient(
                createRecipe: { _ in XCTFail("Expected update, not create") },
                updateRecipe: {
                    capturedUpdate = ($0, $1)
                },
                getRecipe: { _ in throw TestError.unexpectedCall },
                listAllTags: { TagsListResponse(tags: []) },
                uploadPhoto: { _ in UploadPhotoResponse(id: UUID()) }
            )
        )
        viewModel.formData.title = "Soup"
        viewModel.formData.instructions = "Simmer"
        let expectedVersionId = UUID()
        viewModel.formData.expectedVersionId = expectedVersionId

        let didSave = await viewModel.save()

        XCTAssertTrue(didSave)
        XCTAssertEqual(capturedUpdate?.id, recipeId)
        XCTAssertEqual(capturedUpdate?.request.title, "Soup")
        XCTAssertEqual(capturedUpdate?.request.instructions, "Simmer")
        XCTAssertEqual(capturedUpdate?.request.expectedVersionId, expectedVersionId)
    }

    func testUpdateConflictKeepsEditsAndShowsConflictMessage() async {
        let viewModel = RecipeFormViewModel(
            mode: .edit(recipeId: UUID()),
            api: RecipeFormViewAPIClient(
                createRecipe: { _ in XCTFail("Expected update, not create") },
                updateRecipe: { _, _ in
                    throw ErrorResponse.error(
                        409,
                        Data(#"{"code":"conflict","error":"stale"}"#.utf8),
                        nil,
                        TestError.unexpectedCall
                    )
                },
                getRecipe: { _ in throw TestError.unexpectedCall },
                listAllTags: { TagsListResponse(tags: []) },
                uploadPhoto: { _ in UploadPhotoResponse(id: UUID()) }
            )
        )
        viewModel.formData.title = "My unsaved soup"
        viewModel.formData.instructions = "Simmer"
        viewModel.formData.expectedVersionId = UUID()

        let didSave = await viewModel.save()

        XCTAssertFalse(didSave)
        XCTAssertEqual(viewModel.formData.title, "My unsaved soup")
        XCTAssertEqual(
            viewModel.error,
            "This recipe changed since you opened it. Your edits are still here; reload before saving again."
        )
    }

    func testAddTagNormalizesTypedAndNamespacedTags() {
        let viewModel = RecipeFormViewModel(
            mode: .create,
            api: RecipeFormViewAPIClient(
                createRecipe: { _ in },
                updateRecipe: { _, _ in },
                getRecipe: { _ in throw TestError.unexpectedCall },
                listAllTags: { TagsListResponse(tags: []) },
                uploadPhoto: { _ in UploadPhotoResponse(id: UUID()) }
            )
        )

        viewModel.newTagValue = "Meal:Dinner"
        viewModel.addTag()
        viewModel.selectedTagNamespace = ""
        viewModel.newNamespace = "Season"
        viewModel.newTagValue = "Winter"
        viewModel.addTag()
        viewModel.selectedTagNamespace = nil
        viewModel.newTagValue = "season:winter"
        viewModel.addTag()

        XCTAssertEqual(viewModel.formData.tags, ["meal:Dinner", "season:Winter"])
        XCTAssertNil(viewModel.selectedTagNamespace)
        XCTAssertEqual(viewModel.newNamespace, "")
        XCTAssertEqual(viewModel.newTagValue, "")
    }

    private enum TestError: Error {
        case unexpectedCall
    }
}
