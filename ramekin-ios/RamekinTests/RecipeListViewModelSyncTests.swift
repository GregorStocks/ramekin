import XCTest
@testable import Ramekin

/// The paged cache sync: following the server's positions, failure
/// surfacing, and the freshen time budget.
@MainActor
final class RecipeListViewModelSyncTests: XCTestCase {
    private struct SyncRequest: Equatable {
        let cursor: Int64?
        let limit: Int64
        let afterId: UUID?
    }

    func testFirstSyncPagesFromTheStartOfTheStream() async {
        let pageOne = [makeSyncRecipe(title: "Aioli"), makeSyncRecipe(title: "Bread")]
        let pageTwo = [makeSyncRecipe(title: "Curry")]
        var requests: [SyncRequest] = []
        var applied: [SyncRecipesResponse] = []

        let viewModel = makeViewModel(
            syncRecipes: { cursor, limit, afterId in
                requests.append(SyncRequest(cursor: cursor, limit: limit, afterId: afterId))
                if cursor == nil {
                    return SyncRecipesResponse(
                    afterId: pageOne.last?.id,
                    changes: pageOne,
                    cursor: 100,
                    deleted: [],
                    fullResync: true,
                    hasMore: true,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion
                )
                }
                return SyncRecipesResponse(
                    afterId: nil,
                    changes: pageTwo,
                    cursor: 200,
                    deleted: [],
                    fullResync: false,
                    hasMore: false,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion
                )
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                apply: { response, _ in applied.append(response) }
            )
        )

        await viewModel.loadRecipes()

        // Each request is just what the page before it said to ask for.
        XCTAssertEqual(requests, [
            SyncRequest(cursor: nil, limit: 100, afterId: nil),
            SyncRequest(cursor: 100, limit: 100, afterId: pageOne.last?.id)
        ])
        XCTAssertEqual(applied.flatMap { $0.changes.map(\.title) }, ["Aioli", "Bread", "Curry"])
        XCTAssertEqual(applied.map(\.fullResync), [true, false])
        XCTAssertFalse(viewModel.syncFailed)
    }

    func testSyncResumesFromThePositionTheCacheWasLeftAt() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        let resumeAfterId = UUID()
        var requests: [SyncRequest] = []

        let viewModel = makeViewModel(
            syncRecipes: { cursor, limit, afterId in
                requests.append(SyncRequest(cursor: cursor, limit: limit, afterId: afterId))
                return RecipeListTestSupport.emptySyncResponse()
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                // An earlier sync stopped partway through the stream.
                syncPosition: { _ in RecipeSyncPosition(cursor: 50, afterId: resumeAfterId) },
                loadRecipes: { _ in [cached] }
            )
        )

        await viewModel.loadRecipes()

        XCTAssertEqual(requests, [SyncRequest(cursor: 50, limit: 100, afterId: resumeAfterId)])
    }

    func testEmptyCacheSyncsFromTheStartWhateverPositionWasLeft() async {
        var requests: [SyncRequest] = []

        let viewModel = makeViewModel(
            syncRecipes: { cursor, limit, afterId in
                requests.append(SyncRequest(cursor: cursor, limit: limit, afterId: afterId))
                return RecipeListTestSupport.emptySyncResponse()
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) }
            )
        )

        await viewModel.loadRecipes()

        XCTAssertEqual(requests, [SyncRequest(cursor: nil, limit: 100, afterId: nil)])
    }

    func testSyncFailureOverCachedListShowsStaleBannerNotError() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        let viewModel = makeViewModel(
            syncRecipes: { _, _, _ in throw URLError(.timedOut) },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) },
                loadRecipes: { _ in [cached] }
            )
        )

        await viewModel.loadRecipes()

        XCTAssertTrue(viewModel.syncFailed)
        XCTAssertNil(viewModel.error)
        XCTAssertEqual(viewModel.recipes.map(\.id), [cached.id])
        XCTAssertFalse(viewModel.isLoading)
    }

    func testPageClaimingMoreWithoutSayingWhereFailsSync() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        var applied = false
        var requestCount = 0
        let viewModel = makeViewModel(
            syncRecipes: { _, _, _ in
                requestCount += 1
                return SyncRecipesResponse(
                    afterId: nil,
                    changes: [],
                    cursor: 400,
                    deleted: [],
                    fullResync: false,
                    hasMore: true,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion
                )
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) },
                loadRecipes: { _ in [cached] },
                apply: { _, _ in applied = true }
            )
        )

        await viewModel.loadRecipes()

        XCTAssertTrue(viewModel.syncFailed)
        XCTAssertFalse(applied)
        XCTAssertEqual(requestCount, 1)
        XCTAssertEqual(viewModel.recipes.map(\.id), [cached.id])
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
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) },
                loadRecipes: { _ in [cached] }
            )
        )

        await viewModel.loadRecipes()
        XCTAssertTrue(viewModel.syncFailed)

        failSync = false
        await viewModel.loadRecipes()

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
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) },
                loadRecipes: { _ in [cached] }
            ),
            syncBudget: 0.05
        )

        await viewModel.loadRecipes()

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

        await viewModel.loadRecipes()

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
                    changes: [],
                    cursor: 100,
                    deleted: [],
                    fullResync: false,
                    hasMore: false,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion + 1
                )
            },
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) },
                loadRecipes: { _ in [cached] },
                apply: { _, _ in applied = true }
            )
        )

        await viewModel.loadRecipes()

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
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                syncRecipes: syncRecipes
            ),
            cache: cache,
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
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
            photos: [],
            rating: nil,
            tags: [],
            thumbnailPhotoId: nil,
            title: title,
            updatedAt: Date(timeIntervalSince1970: 200)
        )
    }
}
