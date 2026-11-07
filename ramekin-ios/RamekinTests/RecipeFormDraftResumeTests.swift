import XCTest
@testable import Ramekin

/// Resuming a recipe form the app was terminated on.
@MainActor
final class RecipeFormDraftResumeTests: XCTestCase {
    func testResumingNewRecipeRestoresText() {
        let resumed = RecipeFormDraft(mode: .create, form: RecipeFormData(), recipeText: "Pancakes")

        let model = RecipeFormViewModel(mode: .create, resuming: resumed, api: unexpectedCallsClient())

        XCTAssertEqual(model.recipeText, "Pancakes")
        XCTAssertTrue(model.hasUnsavedChanges)
        XCTAssertEqual(model.resumableDraft, resumed)
    }

    func testResumingAfterLosingTrackOfAnImportChecksTheSameJob() async {
        let jobId = UUID()
        let recipeId = UUID()
        let resumed = RecipeFormDraft(
            mode: .create, form: RecipeFormData(), recipeText: "Pancakes", lostImportJobId: jobId
        )
        // importFromText throws: checking again must not start a second import.
        let model = RecipeFormViewModel(mode: .create, resuming: resumed, api: unexpectedCallsClient(
            getScrape: { id in
                XCTAssertEqual(id, jobId)
                return ScrapeJobResponse(
                    canRetry: false,
                    createdAt: Date(timeIntervalSince1970: 1),
                    id: jobId,
                    isTerminal: true,
                    recipeId: recipeId,
                    recipeReady: true,
                    retryCount: 0,
                    status: .completed,
                    steps: []
                )
            }
        ))

        XCTAssertTrue(model.lostTrackOfImport)
        XCTAssertTrue(model.canImport)
        XCTAssertEqual(model.resumableDraft, resumed)

        let saved = await model.importRecipe()

        XCTAssertTrue(saved)
        XCTAssertEqual(model.importedRecipeId, recipeId)
        XCTAssertNil(model.resumableDraft)
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
        getRecipe: @escaping (UUID) async throws -> RecipeResponse = { _ in throw TestError.unexpectedCall },
        getScrape: @escaping (UUID) async throws -> ScrapeJobResponse = { _ in throw TestError.unexpectedCall }
    ) -> RecipeFormViewAPIClient {
        RecipeFormViewAPIClient(
            updateRecipe: { _, _ in throw TestError.unexpectedCall },
            getRecipe: getRecipe,
            syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
            uploadPhoto: { _ in throw TestError.unexpectedCall },
            importFromText: { _ in throw TestError.unexpectedCall },
            getScrape: getScrape
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
