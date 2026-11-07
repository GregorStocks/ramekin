import XCTest
@testable import Ramekin

@MainActor
final class RecipeFormViewModelTests: XCTestCase {
    func testTextImportFollowsTheJobUntilTheRecipeIsSaved() async {
        let jobId = UUID()
        let recipeId = UUID()
        var polls = 0
        var model: RecipeFormViewModel!
        model = RecipeFormViewModel(mode: .create, api: createClient(
            importFromText: { text in
                XCTAssertEqual(text, "Pasted recipe")
                XCTAssertTrue(model.hasUnsavedChanges, "The text isn't safe until the server accepts it")
                return jobId
            },
            getScrape: { id in
                XCTAssertEqual(id, jobId)
                XCTAssertFalse(model.hasUnsavedChanges, "The job has the text")
                polls += 1
                return polls == 1
                    ? makeJob(id: jobId, recipeId: nil, status: "parsing")
                    : makeJob(id: jobId, recipeId: recipeId, status: "normalizing")
            }
        ))
        model.pollIntervalNanoseconds = 0
        XCTAssertFalse(model.canImport)
        model.recipeText = "Pasted recipe"
        XCTAssertTrue(model.canImport)
        XCTAssertFalse(model.canSave, "A new recipe is imported, not saved from the form")

        let saved = await model.importRecipe()

        XCTAssertTrue(saved)
        XCTAssertEqual(polls, 2)
        XCTAssertFalse(model.isImporting)
        XCTAssertNil(model.error)
        XCTAssertEqual(model.importedRecipeId, recipeId)
        XCTAssertFalse(model.canImport)
        XCTAssertFalse(model.hasUnsavedChanges)
    }

    func testLostPollOffersToCheckTheSameJobAgain() async {
        let jobId = UUID()
        let recipeId = UUID()
        var imports = 0
        var pollsFail = true
        var polls = 0
        let model = RecipeFormViewModel(mode: .create, api: createClient(
            importFromText: { _ in
                imports += 1
                return jobId
            },
            getScrape: { id in
                XCTAssertEqual(id, jobId)
                polls += 1
                if pollsFail { throw TestError.unexpectedCall }
                return makeJob(id: jobId, recipeId: recipeId, status: "normalizing")
            }
        ))
        model.pollIntervalNanoseconds = 0
        model.recipeText = "Pasted recipe"

        let saved = await model.importRecipe()

        XCTAssertFalse(saved)
        XCTAssertEqual(polls, 3, "Transient poll errors are retried before giving up")
        XCTAssertEqual(model.error, "Lost track of the import. It may still be running; tap Check Again.")
        XCTAssertTrue(model.lostTrackOfImport)
        XCTAssertTrue(model.canImport, "Check Again follows the same job")
        XCTAssertTrue(model.hasUnsavedChanges, "The text isn't safe until the job is accounted for")

        pollsFail = false
        let savedOnRecheck = await model.importRecipe()

        XCTAssertTrue(savedOnRecheck)
        XCTAssertEqual(imports, 1, "Checking again must not start a second import")
        XCTAssertEqual(model.importedRecipeId, recipeId)
        XCTAssertFalse(model.lostTrackOfImport)
        XCTAssertFalse(model.hasUnsavedChanges)
    }

    func testCompletedJobWithoutRecipeStopsPolling() async {
        let jobId = UUID()
        var polls = 0
        let model = RecipeFormViewModel(mode: .create, api: createClient(
            importFromText: { _ in jobId },
            getScrape: { _ in
                polls += 1
                return makeJob(id: jobId, recipeId: nil, status: "completed")
            }
        ))
        model.pollIntervalNanoseconds = 0
        model.recipeText = "Pasted recipe"

        let saved = await model.importRecipe()

        XCTAssertFalse(saved)
        XCTAssertEqual(polls, 1)
        XCTAssertNotNil(model.error)
        XCTAssertTrue(model.canImport)
        XCTAssertEqual(model.recipeText, "Pasted recipe")
    }

    func testFailedTextImportKeepsTheText() async {
        let jobId = UUID()
        let model = RecipeFormViewModel(mode: .create, api: createClient(
            importFromText: { _ in jobId },
            getScrape: { _ in
                makeJob(id: jobId, recipeId: nil, status: "failed", error: "No recipe found in the text (missing title).")
            }
        ))
        model.recipeText = "My original recipe"

        let saved = await model.importRecipe()

        XCTAssertFalse(saved)
        XCTAssertEqual(model.error, "No recipe found in the text (missing title).")
        XCTAssertEqual(model.recipeText, "My original recipe")
        XCTAssertFalse(model.isImporting)
        XCTAssertTrue(model.canImport)
        XCTAssertTrue(model.hasUnsavedChanges)
    }

    func testTimedOutTextImportKeepsTheTextAndOffersImportAgain() async {
        let jobId = UUID()
        let timeout = "Timed out: extract_recipe was still running after 15 minutes and was stopped"
        var polls = 0
        let model = RecipeFormViewModel(mode: .create, api: createClient(
            importFromText: { _ in jobId },
            getScrape: { _ in
                polls += 1
                return polls < 3
                    ? makeJob(id: jobId, recipeId: nil, status: "parsing")
                    : makeJob(id: jobId, recipeId: nil, status: "failed", error: timeout)
            }
        ))
        model.pollIntervalNanoseconds = 0
        model.recipeText = "My original recipe"

        let saved = await model.importRecipe()

        XCTAssertFalse(saved)
        XCTAssertEqual(polls, 3)
        XCTAssertEqual(model.error, timeout)
        XCTAssertEqual(model.recipeText, "My original recipe")
        XCTAssertTrue(model.canImport)
        XCTAssertTrue(model.hasUnsavedChanges)
    }

    func testTextImportThatCannotStartKeepsTheText() async {
        let model = RecipeFormViewModel(mode: .create, api: createClient(
            importFromText: { _ in throw TestError.unexpectedCall },
            getScrape: { _ in throw TestError.unexpectedCall }
        ))
        model.recipeText = "My original recipe"

        let saved = await model.importRecipe()

        XCTAssertFalse(saved)
        XCTAssertNotNil(model.error)
        XCTAssertEqual(model.recipeText, "My original recipe")
        XCTAssertTrue(model.canImport)
        XCTAssertTrue(model.hasUnsavedChanges)
    }

    func testNormalizeFailureAfterSavingStillSaves() async {
        let jobId = UUID()
        let model = RecipeFormViewModel(mode: .create, api: createClient(
            importFromText: { _ in jobId },
            getScrape: { _ in makeJob(id: jobId, recipeId: UUID(), status: "failed", error: "auto_tag failed") }
        ))
        model.recipeText = "Pasted recipe"

        let saved = await model.importRecipe()

        XCTAssertTrue(saved)
        XCTAssertNil(model.error)
    }

    func testEditSaveRequiresLoadedRecipeVersion() {
        let viewModel = RecipeFormViewModel(
            mode: .edit(recipeId: UUID()),
            api: RecipeFormViewAPIClient(
                updateRecipe: { _, _ in throw TestError.unexpectedCall },
                getRecipe: { _ in throw TestError.unexpectedCall },
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                uploadPhoto: { _ in throw TestError.unexpectedCall }
            )
        )
        viewModel.formData.title = "Soup"
        viewModel.formData.instructions = "Simmer"

        XCTAssertFalse(viewModel.canSave)

        viewModel.formData.expectedVersionId = UUID()

        XCTAssertTrue(viewModel.canSave)
    }

    func testUpdateSaveUsesExistingRecipeId() async {
        let recipeId = UUID()
        var capturedUpdate: (id: UUID, request: UpdateRecipeRequest)?
        let viewModel = RecipeFormViewModel(
            mode: .edit(recipeId: recipeId),
            api: RecipeFormViewAPIClient(
                updateRecipe: {
                    capturedUpdate = ($0, $1)
                },
                getRecipe: { _ in throw TestError.unexpectedCall },
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
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
                updateRecipe: { _, _ in
                    throw ErrorResponse.error(
                        409,
                        Data(#"{"code":"conflict","error":"stale"}"#.utf8),
                        nil,
                        TestError.unexpectedCall
                    )
                },
                getRecipe: { _ in throw TestError.unexpectedCall },
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
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
                updateRecipe: { _, _ in },
                getRecipe: { _ in throw TestError.unexpectedCall },
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
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

    func testCreateHasUnsavedChangesOnceTextIsTyped() {
        let model = RecipeFormViewModel(mode: .create, api: createClient(
            importFromText: { _ in throw TestError.unexpectedCall },
            getScrape: { _ in throw TestError.unexpectedCall }
        ))
        XCTAssertFalse(model.hasUnsavedChanges)
        model.recipeText = "  \n"
        XCTAssertFalse(model.hasUnsavedChanges)
        model.recipeText = "Pancakes"
        XCTAssertTrue(model.hasUnsavedChanges)
    }

    func testEditHasUnsavedChangesOnlyAfterAnEdit() async {
        let recipe = RecipeResponse(
            cookTime: nil,
            createdAt: Date(timeIntervalSince1970: 100),
            derivedMeasurements: [],
            description: nil,
            difficulty: nil,
            id: UUID(),
            ingredients: [],
            instructions: "Mix",
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
            title: "Plain",
            totalTime: nil,
            updatedAt: Date(timeIntervalSince1970: 200),
            versionId: UUID(),
            versionSource: "user"
        )
        let model = RecipeFormViewModel(mode: .edit(recipeId: recipe.id), api: RecipeFormViewAPIClient(
            updateRecipe: { _, _ in throw TestError.unexpectedCall },
            getRecipe: { _ in recipe },
            syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
            uploadPhoto: { _ in throw TestError.unexpectedCall }
        ))
        XCTAssertFalse(model.hasUnsavedChanges, "Nothing to lose before the recipe loads")
        await model.loadRecipe(id: recipe.id)
        XCTAssertFalse(model.hasUnsavedChanges)
        model.formData.title = "Plainer"
        XCTAssertTrue(model.hasUnsavedChanges)
        model.formData.title = "Plain"
        XCTAssertFalse(model.hasUnsavedChanges)
        model.isUploadingPhoto = true
        XCTAssertTrue(model.hasUnsavedChanges, "A pending photo upload would be lost")
    }
}

private enum TestError: Error {
    case unexpectedCall
}

@MainActor
private func createClient(
    importFromText: @escaping (String) async throws -> UUID,
    getScrape: @escaping (UUID) async throws -> ScrapeJobResponse
) -> RecipeFormViewAPIClient {
    RecipeFormViewAPIClient(
        updateRecipe: { _, _ in throw TestError.unexpectedCall },
        getRecipe: { _ in throw TestError.unexpectedCall },
        syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
        uploadPhoto: { _ in throw TestError.unexpectedCall },
        importFromText: importFromText,
        getScrape: getScrape
    )
}

/// A job as the server reports it, with `isTerminal` and `recipeReady`
/// following the server's rules for `status`.
private func makeJob(
    id: UUID,
    recipeId: UUID?,
    status rawStatus: String,
    error: String? = nil
) -> ScrapeJobResponse {
    let status = ScrapeJobStatus(rawValue: rawStatus)!
    let stopped = status == .completed || status == .failed
    return ScrapeJobResponse(
        canRetry: status == .failed,
        createdAt: Date(timeIntervalSince1970: 1),
        error: error,
        id: id,
        isTerminal: stopped,
        recipeId: recipeId,
        recipeReady: recipeId != nil && (stopped || status == .normalizing),
        retryCount: 0,
        status: status,
        steps: []
    )
}
