import XCTest
@testable import Ramekin

/// The paged cache sync: durable sweep progress, first-page watermark
/// persistence, failure surfacing, and the freshen time budget.
@MainActor
final class RecipeListViewModelSyncTests: XCTestCase {
    private struct SyncRequest: Equatable {
        let cursor: Int64?
        let limit: Int64
        let afterId: UUID?
    }

    func testMultiPageSweepPersistsFirstPageWatermark() async {
        let pageOne = [makeSyncRecipe(title: "Aioli"), makeSyncRecipe(title: "Bread")]
        let pageTwo = [makeSyncRecipe(title: "Curry")]
        var requests: [SyncRequest] = []
        var persistedSweeps: [PendingSyncSweep] = []
        var persistedCursor: Int64?
        var clearedPendingSweep = false
        var appliedRecipeTitles: [String] = []

        let viewModel = makeViewModel(
            syncRecipes: { cursor, limit, afterId in
                requests.append(SyncRequest(cursor: cursor, limit: limit, afterId: afterId))
                // The second page's snapshot watermark is higher: changes
                // committed mid-sweep sit between the two, and only the first
                // watermark redelivers them on the next sync.
                if afterId == nil {
                    return SyncRecipesResponse(
                        cursor: 100,
                        deleted: [],
                        hasMore: true,
                        normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                        recipes: pageOne
                    )
                }
                return SyncRecipesResponse(
                    cursor: 200,
                    deleted: [],
                    hasMore: false,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                    recipes: pageTwo
                )
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                setSyncCursor: { cursor, _ in persistedCursor = cursor },
                setPendingSyncSweep: { sweep, _ in persistedSweeps.append(sweep) },
                clearPendingSyncSweep: { _ in clearedPendingSweep = true },
                apply: { response, _ in
                    appliedRecipeTitles.append(contentsOf: response.recipes.map(\.title))
                }
            )
        )

        await viewModel.loadRecipes(reset: true)

        XCTAssertEqual(requests, [
            SyncRequest(cursor: nil, limit: 100, afterId: nil),
            SyncRequest(cursor: nil, limit: 100, afterId: pageOne.last?.id)
        ])
        XCTAssertEqual(appliedRecipeTitles, ["Aioli", "Bread", "Curry"])
        XCTAssertEqual(
            persistedSweeps,
            [PendingSyncSweep(since: nil, afterId: pageOne.last!.id, watermark: 100)]
        )
        XCTAssertEqual(persistedCursor, 100)
        XCTAssertTrue(clearedPendingSweep)
        XCTAssertFalse(viewModel.syncFailed)
    }

    func testInterruptedSweepResumesFromPendingState() async {
        let resumeAfterId = UUID()
        let finalPage = [makeSyncRecipe(title: "Dal")]
        var requests: [SyncRequest] = []
        var persistedCursor: Int64?

        let viewModel = makeViewModel(
            syncRecipes: { cursor, limit, afterId in
                requests.append(SyncRequest(cursor: cursor, limit: limit, afterId: afterId))
                return SyncRecipesResponse(
                    cursor: 500,
                    deleted: [],
                    hasMore: false,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                    recipes: finalPage
                )
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                setSyncCursor: { cursor, _ in persistedCursor = cursor },
                pendingSyncSweep: { _ in
                    PendingSyncSweep(since: nil, afterId: resumeAfterId, watermark: 50)
                }
            )
        )

        await viewModel.loadRecipes(reset: true)

        XCTAssertEqual(requests, [SyncRequest(cursor: nil, limit: 100, afterId: resumeAfterId)])
        // The resumed sweep persists the original first-page watermark, not
        // this page's: pages fetched before the interruption ran against the
        // older snapshot.
        XCTAssertEqual(persistedCursor, 50)
    }

    func testPendingStateForDifferentCursorIsIgnored() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        var requests: [SyncRequest] = []

        let viewModel = makeViewModel(
            syncRecipes: { cursor, limit, afterId in
                requests.append(SyncRequest(cursor: cursor, limit: limit, afterId: afterId))
                return RecipeListTestSupport.emptySyncResponse()
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in 300 },
                pendingSyncSweep: { _ in
                    // Left over from a full sync; this sweep filters on 300.
                    PendingSyncSweep(since: nil, afterId: UUID(), watermark: 50)
                },
                loadRecipes: { _ in [cached] }
            )
        )

        await viewModel.loadRecipes(reset: true)

        XCTAssertEqual(requests, [SyncRequest(cursor: 300, limit: 100, afterId: nil)])
    }

    func testSyncFailureOverCachedListShowsStaleBannerNotError() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        let viewModel = makeViewModel(
            syncRecipes: { _, _, _ in throw URLError(.timedOut) },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in 300 },
                loadRecipes: { _ in [cached] }
            )
        )

        await viewModel.loadRecipes(reset: true)

        XCTAssertTrue(viewModel.syncFailed)
        XCTAssertNil(viewModel.error)
        XCTAssertEqual(viewModel.recipes.map(\.id), [cached.id])
        XCTAssertFalse(viewModel.isLoading)
    }

    func testSuccessfulRetryClearsStaleBanner() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        var failSync = true
        let viewModel = makeViewModel(
            syncRecipes: { _, _, _ in
                if failSync { throw URLError(.timedOut) }
                return RecipeListTestSupport.emptySyncResponse()
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in 300 },
                loadRecipes: { _ in [cached] }
            )
        )

        await viewModel.loadRecipes(reset: true)
        XCTAssertTrue(viewModel.syncFailed)

        failSync = false
        await viewModel.loadRecipes(reset: true)

        XCTAssertFalse(viewModel.syncFailed)
    }

    func testFreshenSyncExceedingBudgetShowsStaleBanner() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        let viewModel = makeViewModel(
            syncRecipes: { _, _, _ in
                try await Task.sleep(nanoseconds: 5_000_000_000)
                return RecipeListTestSupport.emptySyncResponse()
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in 300 },
                loadRecipes: { _ in [cached] }
            ),
            syncBudget: 0.05
        )

        await viewModel.loadRecipes(reset: true)

        XCTAssertTrue(viewModel.syncFailed)
        XCTAssertNil(viewModel.error)
        XCTAssertEqual(viewModel.recipes.map(\.id), [cached.id])
    }

    func testFirstSyncWithEmptyCacheGetsNoBudget() async {
        // An empty cache means nothing is on screen yet — cancelling the
        // sweep would leave the user with a spinner and no data, so only
        // freshens are budgeted.
        var didSync = false
        let viewModel = makeViewModel(
            syncRecipes: { _, _, _ in
                try await Task.sleep(nanoseconds: 200_000_000)
                didSync = true
                return RecipeListTestSupport.emptySyncResponse()
            },
            cache: RecipeListTestSupport.cacheClient(currentAccountKey: { "account" }),
            syncBudget: 0.05
        )

        await viewModel.loadRecipes(reset: true)

        XCTAssertTrue(didSync)
        XCTAssertFalse(viewModel.syncFailed)
    }

    func testSyncFailsOnNormalizationContractMismatch() async {
        // A server whose search contract moved must not populate the cache:
        // recipes normalized under a different contract would make local
        // search silently disagree with server search.
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        var applied = false
        let viewModel = makeViewModel(
            syncRecipes: { _, _, _ in
                SyncRecipesResponse(
                    cursor: 100,
                    deleted: [],
                    hasMore: false,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion + 1,
                    recipes: []
                )
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in 300 },
                loadRecipes: { _ in [cached] },
                apply: { _, _ in applied = true }
            )
        )

        await viewModel.loadRecipes(reset: true)

        XCTAssertFalse(applied)
        XCTAssertTrue(viewModel.syncFailed)
        XCTAssertEqual(viewModel.recipes.map(\.id), [cached.id])
    }

    private func makeViewModel(
        syncRecipes: @escaping (Int64?, Int64, UUID?) async throws -> SyncRecipesResponse,
        cache: RecipeListCacheClient,
        syncBudget: TimeInterval = 15
    ) -> RecipeListViewModel {
        RecipeListViewModel(
            api: RecipeListViewAPIClient(
                listAllTags: { TagsListResponse(tags: []) },
                listRecipes: { limit, offset, _, _, _ in
                    ListRecipesResponse(
                        pagination: PaginationMetadata(limit: limit, offset: offset, total: 0),
                        recipes: []
                    )
                },
                syncRecipes: syncRecipes
            ),
            cache: cache,
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20,
            syncBudget: syncBudget
        )
    }

    private func makeSyncRecipe(title: String) -> SyncRecipe {
        SyncRecipe(
            createdAt: Date(timeIntervalSince1970: 100),
            description: nil,
            id: UUID(),
            ingredientMatchText: "[]",
            ingredients: [],
            instructions: "Cook it.",
            notes: nil,
            rating: nil,
            tags: [],
            thumbnailPhotoId: nil,
            title: title,
            updatedAt: Date(timeIntervalSince1970: 200)
        )
    }
}
