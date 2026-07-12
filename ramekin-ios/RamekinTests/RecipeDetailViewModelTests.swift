import UIKit
import XCTest
@testable import Ramekin

final class RecipeDetailViewModelTests: XCTestCase {
    @MainActor
    func testLoadRecipeStoresCurrentRecipeAndVersionId() async {
        let recipeId = UUID()
        let currentVersionId = UUID()
        let recipe = makeRecipe(id: recipeId, versionId: currentVersionId)
        var requestedVersionId: UUID?

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { id, versionId in
                    XCTAssertEqual(id, recipeId)
                    requestedVersionId = versionId
                    return recipe
                }
            )
        )

        await viewModel.loadRecipe()

        XCTAssertEqual(viewModel.recipe?.id, recipeId)
        XCTAssertEqual(viewModel.currentVersionId, currentVersionId)
        XCTAssertNil(requestedVersionId)
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertNil(viewModel.error)
    }

    @MainActor
    func testLoadHistoricalRecipeFetchesCurrentVersionIdWhenMissing() async {
        let recipeId = UUID()
        let historicalVersionId = UUID()
        let currentVersionId = UUID()
        let historicalRecipe = makeRecipe(id: recipeId, versionId: historicalVersionId)
        let currentRecipe = makeRecipe(id: recipeId, versionId: currentVersionId)
        var requestedVersionIds: [UUID?] = []

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, versionId in
                    requestedVersionIds.append(versionId)
                    return versionId == historicalVersionId ? historicalRecipe : currentRecipe
                }
            )
        )

        await viewModel.loadRecipe(versionId: historicalVersionId)

        XCTAssertEqual(requestedVersionIds, [historicalVersionId, nil])
        XCTAssertEqual(viewModel.recipe?.versionId, historicalVersionId)
        XCTAssertEqual(viewModel.currentVersionId, currentVersionId)
        XCTAssertTrue(viewModel.isViewingHistoricalVersion)
    }

    @MainActor
    func testLoadVersionHistoryStoresVersionsAndCurrentVersion() async {
        let recipeId = UUID()
        let current = makeVersion(isCurrent: true)
        let older = makeVersion(isCurrent: false)

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                listVersions: { id in
                    XCTAssertEqual(id, recipeId)
                    return VersionListResponse(versions: [older, current])
                }
            )
        )

        await viewModel.loadVersionHistory()

        XCTAssertEqual(viewModel.versionHistory.map(\.id), [older.id, current.id])
        XCTAssertEqual(viewModel.currentVersionId, current.id)
        XCTAssertFalse(viewModel.isLoadingVersions)
        XCTAssertNil(viewModel.versionHistoryError)
    }

    @MainActor
    func testOpenCompareSheetLoadsSelectedVersionsInDateOrder() async {
        let recipeId = UUID()
        let firstVersionId = UUID()
        let secondVersionId = UUID()
        let older = makeRecipe(
            id: recipeId,
            versionId: secondVersionId,
            updatedAt: Date(timeIntervalSince1970: 100)
        )
        let newer = makeRecipe(
            id: recipeId,
            versionId: firstVersionId,
            updatedAt: Date(timeIntervalSince1970: 200)
        )

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, versionId in
                    versionId == firstVersionId ? newer : older
                }
            )
        )
        viewModel.compareSelection = [firstVersionId, secondVersionId]

        await viewModel.openCompareSheet()

        XCTAssertTrue(viewModel.showingCompareSheet)
        XCTAssertEqual(viewModel.comparedOlderVersion?.versionId, secondVersionId)
        XCTAssertEqual(viewModel.comparedNewerVersion?.versionId, firstVersionId)
        XCTAssertFalse(viewModel.isLoadingCompare)
        XCTAssertNil(viewModel.compareError)
    }

    @MainActor
    func testOpenCompareSheetUsesInitialSelectionAfterAwait() async {
        let recipeId = UUID()
        let firstVersionId = UUID()
        let secondVersionId = UUID()
        let replacementVersionId = UUID()
        let older = makeRecipe(
            id: recipeId,
            versionId: secondVersionId,
            updatedAt: Date(timeIntervalSince1970: 100)
        )
        let newer = makeRecipe(
            id: recipeId,
            versionId: firstVersionId,
            updatedAt: Date(timeIntervalSince1970: 200)
        )
        var requestedVersionIds: [UUID?] = []
        var firstFetchContinuation: CheckedContinuation<RecipeResponse, Never>?

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, versionId in
                    requestedVersionIds.append(versionId)
                    if versionId == firstVersionId {
                        return await withCheckedContinuation { continuation in
                            firstFetchContinuation = continuation
                        }
                    }
                    return older
                }
            )
        )
        viewModel.compareSelection = [firstVersionId, secondVersionId]

        let compareTask = Task { await viewModel.openCompareSheet() }
        while firstFetchContinuation == nil {
            await Task.yield()
        }
        viewModel.compareSelection = [replacementVersionId]
        firstFetchContinuation?.resume(returning: newer)

        await compareTask.value

        XCTAssertEqual(requestedVersionIds, [firstVersionId, secondVersionId])
        XCTAssertEqual(viewModel.comparedOlderVersion?.versionId, secondVersionId)
        XCTAssertEqual(viewModel.comparedNewerVersion?.versionId, firstVersionId)
    }

    @MainActor
    func testCustomScaleOnlyAcceptsPositiveDecimalValues() {
        let viewModel = RecipeDetailViewModel(recipeId: UUID(), api: makeAPI())

        viewModel.customScaleInput = "2.5"
        viewModel.applyCustomScale()
        XCTAssertEqual(viewModel.recipeScale, 2.5)

        viewModel.customScaleInput = "-1"
        viewModel.applyCustomScale()
        XCTAssertEqual(viewModel.recipeScale, 2.5)
    }

    @MainActor
    func testApplyEnrichmentUpdatesRecipeAndClearsPreview() async {
        let recipeId = UUID()
        let original = makeRecipe(id: recipeId, title: "Original")
        let updated = makeRecipe(id: recipeId, title: "Updated")
        let modified = RecipeContent(
            ingredients: [],
            instructions: "Updated instructions",
            tags: ["dinner"],
            title: "Updated"
        )
        var updateRequest: UpdateRecipeRequest?
        var loadCount = 0

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, _ in
                    loadCount += 1
                    return loadCount == 1 ? original : updated
                },
                updateRecipe: { id, request in
                    XCTAssertEqual(id, recipeId)
                    updateRequest = request
                }
            )
        )
        viewModel.enrichResult = modified

        await viewModel.loadRecipe()
        await viewModel.applyEnrichment(modified)

        XCTAssertEqual(updateRequest?.title, "Updated")
        XCTAssertEqual(updateRequest?.instructions, "Updated instructions")
        XCTAssertEqual(updateRequest?.expectedVersionId, original.versionId)
        XCTAssertEqual(viewModel.recipe?.title, "Updated")
        XCTAssertNil(viewModel.enrichResult)
    }

    @MainActor
    func testGenerateDescriptionReloadsRecipeOnlyWhenChanged() async {
        let recipeId = UUID()
        var loadCount = 0

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                generateDescription: { id in
                    XCTAssertEqual(id, recipeId)
                    return GenerateDescriptionResponse(
                        cached: false,
                        changed: true,
                        generatedDescription: "Generated"
                    )
                },
                getRecipe: { _, _ in
                    loadCount += 1
                    return makeRecipe(id: recipeId, title: "Reloaded")
                }
            )
        )
        viewModel.recipe = makeRecipe(id: recipeId, title: "Original")

        await viewModel.generateDescription()

        XCTAssertEqual(loadCount, 1)
        XCTAssertEqual(viewModel.recipe?.title, "Reloaded")
        XCTAssertFalse(viewModel.isGeneratingDescription)
        XCTAssertNil(viewModel.autoEnrichError)
    }
}

private enum UnexpectedAPICall: Error {
    case unexpected
}

private func makeAPI(
    deleteRecipe: @escaping (UUID) async throws -> Void = { _ in throw UnexpectedAPICall.unexpected },
    enrichRecipe: @escaping (RecipeContent) async throws -> RecipeContent = { _ in throw UnexpectedAPICall.unexpected },
    exportRecipe: @escaping (UUID) async throws -> RamekinAPI.ExportDownload = { _ in throw UnexpectedAPICall.unexpected },
    fetchCoverPhoto: @escaping (RecipeResponse) async -> UIImage? = { _ in nil },
    generateDescription: @escaping (UUID) async throws -> GenerateDescriptionResponse = { _ in throw UnexpectedAPICall.unexpected },
    generatePhoto: @escaping (UUID) async throws -> GeneratePhotoResponse = { _ in throw UnexpectedAPICall.unexpected },
    getRecipe: @escaping (UUID, UUID?) async throws -> RecipeResponse = { _, _ in throw UnexpectedAPICall.unexpected },
    getScrape: @escaping (UUID) async throws -> ScrapeJobResponse = { _ in throw UnexpectedAPICall.unexpected },
    listVersions: @escaping (UUID) async throws -> VersionListResponse = { _ in throw UnexpectedAPICall.unexpected },
    normalizeTitle: @escaping (UUID) async throws -> NormalizeTitleResponse = { _ in throw UnexpectedAPICall.unexpected },
    rescrape: @escaping (UUID) async throws -> RescrapeResponse = { _ in throw UnexpectedAPICall.unexpected },
    revertRecipe: @escaping (UUID, RecipeResponse, UUID) async throws -> Void = { _, _, _ in throw UnexpectedAPICall.unexpected },
    updateRecipe: @escaping (UUID, UpdateRecipeRequest) async throws -> Void = { _, _ in throw UnexpectedAPICall.unexpected }
) -> RecipeDetailViewAPIClient {
    RecipeDetailViewAPIClient(
        deleteRecipe: deleteRecipe,
        enrichRecipe: enrichRecipe,
        exportRecipe: exportRecipe,
        fetchCoverPhoto: fetchCoverPhoto,
        generateDescription: generateDescription,
        generatePhoto: generatePhoto,
        getRecipe: getRecipe,
        getScrape: getScrape,
        listVersions: listVersions,
        normalizeTitle: normalizeTitle,
        rescrape: rescrape,
        revertRecipe: revertRecipe,
        updateRecipe: updateRecipe
    )
}

private func makeRecipe(
    id: UUID = UUID(),
    title: String = "Recipe",
    versionId: UUID = UUID(),
    updatedAt: Date = Date(timeIntervalSince1970: 100)
) -> RecipeResponse {
    RecipeResponse(
        cookTime: nil,
        createdAt: Date(timeIntervalSince1970: 1),
        description: nil,
        difficulty: nil,
        id: id,
        ingredients: [],
        instructions: "Mix.",
        notes: nil,
        nutritionalInfo: nil,
        photoIds: [],
        prepTime: nil,
        rating: nil,
        servings: nil,
        sourceName: nil,
        sourceUrl: nil,
        tags: [],
        title: title,
        totalTime: nil,
        updatedAt: updatedAt,
        versionId: versionId,
        versionSource: "user"
    )
}

private func makeVersion(isCurrent: Bool) -> VersionSummary {
    VersionSummary(
        createdAt: Date(),
        id: UUID(),
        isCurrent: isCurrent,
        title: isCurrent ? "Current" : "Older",
        versionSource: "user"
    )
}
