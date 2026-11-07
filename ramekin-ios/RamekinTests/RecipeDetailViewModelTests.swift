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
        XCTAssertNil(viewModel.loadError)
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

        XCTAssertEqual(viewModel.activeSheet, .compare)
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
    func testClosingCompareMidLoadDropsTheLoad() async {
        let recipeId = UUID()
        let firstVersionId = UUID()
        let secondVersionId = UUID()
        var firstFetchContinuation: CheckedContinuation<RecipeResponse, Never>?

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, versionId in
                    if versionId == firstVersionId {
                        return await withCheckedContinuation { firstFetchContinuation = $0 }
                    }
                    return makeRecipe(id: recipeId, versionId: secondVersionId)
                }
            )
        )
        viewModel.compareSelection = [firstVersionId, secondVersionId]

        let compareTask = Task { await viewModel.openCompareSheet() }
        while firstFetchContinuation == nil {
            await Task.yield()
        }
        // A swipe-down sets the sheet binding to nil.
        viewModel.activeSheet = nil
        XCTAssertFalse(viewModel.isLoadingCompare)
        firstFetchContinuation?.resume(returning: makeRecipe(id: recipeId, versionId: firstVersionId))
        await compareTask.value

        XCTAssertNil(viewModel.activeSheet)
        XCTAssertNil(viewModel.comparedOlderVersion)
        XCTAssertNil(viewModel.comparedNewerVersion)
        XCTAssertFalse(viewModel.isLoadingCompare)
    }

    @MainActor
    func testStaleLoadDoesNotOverwriteANewerOne() async {
        let recipeId = UUID()
        let historicalVersionId = UUID()
        let currentVersionId = UUID()
        var historicalContinuation: CheckedContinuation<RecipeResponse, Never>?

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, versionId in
                    if versionId == historicalVersionId {
                        return await withCheckedContinuation { historicalContinuation = $0 }
                    }
                    return makeRecipe(id: recipeId, versionId: currentVersionId)
                }
            )
        )

        let staleLoad = Task { await viewModel.loadRecipe(versionId: historicalVersionId) }
        while historicalContinuation == nil {
            await Task.yield()
        }
        await viewModel.loadRecipe()
        historicalContinuation?.resume(returning: makeRecipe(id: recipeId, versionId: historicalVersionId))
        await staleLoad.value

        XCTAssertEqual(viewModel.recipe?.versionId, currentVersionId)
        XCTAssertFalse(viewModel.isViewingHistoricalVersion)
        XCTAssertFalse(viewModel.isLoading)
    }

    @MainActor
    func testLoadStaysLoadingUntilItsFollowUpFetchFinishes() async {
        let recipeId = UUID()
        let historicalVersionId = UUID()
        var currentContinuation: CheckedContinuation<RecipeResponse, Never>?

        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, versionId in
                    if versionId == nil {
                        return await withCheckedContinuation { currentContinuation = $0 }
                    }
                    return makeRecipe(id: recipeId, versionId: historicalVersionId)
                }
            )
        )

        let load = Task { await viewModel.loadRecipe(versionId: historicalVersionId) }
        while currentContinuation == nil {
            await Task.yield()
        }
        XCTAssertNotNil(viewModel.recipe)
        XCTAssertTrue(viewModel.isLoading)

        currentContinuation?.resume(returning: makeRecipe(id: recipeId))
        await load.value
        XCTAssertFalse(viewModel.isLoading)
    }

    @MainActor
    func testFailedRevertAlertsAndKeepsTheLoadErrorClear() async {
        let recipeId = UUID()
        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, _ in makeRecipe(id: recipeId) },
                revertRecipe: { _, _, _ in throw URLError(.timedOut) }
            )
        )
        await viewModel.loadRecipe()

        await viewModel.revert(to: makeVersion(isCurrent: false))

        XCTAssertEqual(viewModel.actionAlert?.title, "Revert Failed")
        XCTAssertNil(viewModel.loadError)
        XCTAssertNil(viewModel.runningAction)
    }

    @MainActor
    func testSuccessfulRevertReloadsTheRecipe() async {
        let recipeId = UUID()
        var loads = 0
        let viewModel = RecipeDetailViewModel(
            recipeId: recipeId,
            api: makeAPI(
                getRecipe: { _, versionId in
                    if versionId == nil { loads += 1 }
                    return makeRecipe(id: recipeId, title: "Load \(loads)")
                },
                revertRecipe: { _, _, _ in }
            )
        )
        await viewModel.loadRecipe()

        await viewModel.revert(to: makeVersion(isCurrent: false))

        XCTAssertEqual(loads, 2)
        XCTAssertNil(viewModel.actionAlert)
        XCTAssertFalse(viewModel.isReverting)
    }

    @MainActor
    func testDeleteReportsFailuresButNotCancellation() async {
        var failure: Error = CancellationError()
        let viewModel = RecipeDetailViewModel(
            recipeId: UUID(),
            api: makeAPI(deleteRecipe: { _ in throw failure })
        )

        let cancelled = await viewModel.deleteRecipe()
        XCTAssertFalse(cancelled)
        XCTAssertNil(viewModel.actionAlert)
        XCTAssertFalse(viewModel.isDeleting)

        failure = URLError(.timedOut)
        let failed = await viewModel.deleteRecipe()
        XCTAssertFalse(failed)
        XCTAssertEqual(viewModel.actionAlert?.title, "Delete Failed")
        XCTAssertFalse(viewModel.isDeleting)
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

        viewModel.customScaleInput = "1000001"
        viewModel.applyCustomScale()
        XCTAssertEqual(viewModel.recipeScale, 2.5)
        viewModel.setRecipeScale(1e300)
        XCTAssertEqual(viewModel.recipeScale, 2.5)
        viewModel.setRecipeScale(1_000_000)
        XCTAssertEqual(viewModel.recipeScale, 1_000_000)
    }
}

private enum UnexpectedAPICall: Error {
    case unexpected
}

private func makeAPI(
    deleteRecipe: @escaping (UUID) async throws -> Void = { _ in throw UnexpectedAPICall.unexpected },
    exportRecipe: @escaping (UUID) async throws -> RamekinAPI.ExportDownload = { _ in throw UnexpectedAPICall.unexpected },
    fetchCoverPhoto: @escaping (RecipeResponse) async -> UIImage? = { _ in nil },
    generatePhoto: @escaping (UUID) async throws -> GeneratePhotoResponse = { _ in throw UnexpectedAPICall.unexpected },
    getRecipe: @escaping (UUID, UUID?) async throws -> RecipeResponse = { _, _ in throw UnexpectedAPICall.unexpected },
    listVersions: @escaping (UUID) async throws -> VersionListResponse = { _ in throw UnexpectedAPICall.unexpected },
    revertRecipe: @escaping (UUID, RecipeResponse, UUID) async throws -> Void = { _, _, _ in throw UnexpectedAPICall.unexpected }
) -> RecipeDetailViewAPIClient {
    RecipeDetailViewAPIClient(
        deleteRecipe: deleteRecipe,
        exportRecipe: exportRecipe,
        fetchCoverPhoto: fetchCoverPhoto,
        generatePhoto: generatePhoto,
        getRecipe: getRecipe,
        listVersions: listVersions,
        revertRecipe: revertRecipe
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
        derivedMeasurements: [],
        description: nil,
        difficulty: nil,
        id: id,
        ingredients: [],
        instructions: "Mix.",
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
        normalizeOps: [],
        title: isCurrent ? "Current" : "Older",
        versionSource: "user"
    )
}
