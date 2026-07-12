import XCTest
@testable import Ramekin

@MainActor
final class RecipeListViewModelTests: XCTestCase {
    func testTextSearchUsesRelevanceSortAndStoresResults() async {
        let recipe = RecipeListTestSupport.makeRecipe(title: "Pasta")
        var capturedRequest: ListRequest?
        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                listAllTags: { TagsListResponse(tags: []) },
                listRecipes: { limit, offset, query, sortBy, sortDir in
                    capturedRequest = ListRequest(
                        limit: limit,
                        offset: offset,
                        query: query,
                        sortBy: sortBy,
                        sortDir: sortDir
                    )
                    return ListRecipesResponse(
                        pagination: PaginationMetadata(limit: limit, offset: offset, total: 1),
                        recipes: [recipe]
                    )
                },
                syncRecipes: { _ in
                    SyncRecipesResponse(cursor: 1, deleted: [], recipes: [])
                }
            ),
            cache: RecipeListTestSupport.noCacheClient(),
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20
        )

        viewModel.searchText = "pasta"
        await viewModel.loadRecipes(reset: true)

        XCTAssertEqual(viewModel.recipes.map(\.id), [recipe.id])
        XCTAssertEqual(capturedRequest?.limit, 20)
        XCTAssertEqual(capturedRequest?.offset, 0)
        XCTAssertEqual(capturedRequest?.query, "pasta")
        XCTAssertNil(capturedRequest?.sortBy)
        XCTAssertNil(capturedRequest?.sortDir)
        XCTAssertFalse(viewModel.hasMore)
        XCTAssertFalse(viewModel.isLoading)
    }

    func testCacheableBrowseLoadsAndFiltersSyncedCache() async {
        let matching = RecipeListTestSupport.makeRecipe(title: "Dinner", tags: ["Dinner"])
        let hidden = RecipeListTestSupport.makeRecipe(title: "Lunch", tags: ["Lunch"])
        var didListRecipes = false
        var didSync = false
        var cachedRecipes = [matching, hidden]

        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                listAllTags: { TagsListResponse(tags: []) },
                listRecipes: { _, _, _, _, _ in
                    didListRecipes = true
                    return ListRecipesResponse(
                        pagination: PaginationMetadata(limit: 20, offset: 0, total: 0),
                        recipes: []
                    )
                },
                syncRecipes: { _ in
                    didSync = true
                    return SyncRecipesResponse(cursor: 1, deleted: [], recipes: [])
                }
            ),
            cache: RecipeListCacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in nil },
                clearSyncCursor: { _ in },
                loadRecipes: { _ in cachedRecipes },
                apply: { _, _ in cachedRecipes = [matching, hidden] }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20
        )

        viewModel.selectedTags = ["Dinner"]
        await viewModel.loadRecipes(reset: true)

        XCTAssertTrue(didSync)
        XCTAssertFalse(didListRecipes)
        XCTAssertEqual(viewModel.recipes.map(\.id), [matching.id])
        XCTAssertEqual(viewModel.totalCount, 1)
        XCTAssertTrue(viewModel.isUsingLocalCache)
        XCTAssertFalse(viewModel.hasMore)
    }

    func testClearFiltersClearsSearchAndPersistedFilters() {
        let viewModel = RecipeListViewModel(
            api: RecipeListTestSupport.emptyAPIClient(),
            cache: RecipeListTestSupport.noCacheClient(),
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20
        )

        viewModel.searchText = "cake"
        viewModel.selectedTags = ["Dessert"]
        viewModel.photoFilter = .hasPhotos
        viewModel.sourceFilter = "Bakery"
        viewModel.createdAfterFilter = "2024-01-01"
        viewModel.photoSizeFilter = ">1000"

        viewModel.clearFilters()

        XCTAssertEqual(viewModel.searchText, "")
        XCTAssertTrue(viewModel.selectedTags.isEmpty)
        XCTAssertEqual(viewModel.photoFilter, .any)
        XCTAssertEqual(viewModel.sourceFilter, "")
        XCTAssertEqual(viewModel.createdAfterFilter, "")
        XCTAssertEqual(viewModel.photoSizeFilter, "")
    }

    func testClearingSearchAppliesCachedRecipesWhenSyncFails() async {
        let cachedOld = RecipeListTestSupport.makeRecipe(
            title: "Aioli",
            updatedAt: Date(timeIntervalSince1970: 100)
        )
        let cachedNew = RecipeListTestSupport.makeRecipe(
            title: "Zuppa Toscana",
            updatedAt: Date(timeIntervalSince1970: 200)
        )
        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                listAllTags: { TagsListResponse(tags: []) },
                listRecipes: { limit, offset, _, _, _ in
                    ListRecipesResponse(
                        pagination: PaginationMetadata(limit: limit, offset: offset, total: 1),
                        recipes: [cachedNew]
                    )
                },
                syncRecipes: { _ in throw URLError(.timedOut) }
            ),
            cache: RecipeListCacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in 300 },
                clearSyncCursor: { _ in },
                loadRecipes: { _ in [cachedOld, cachedNew] },
                apply: { _, _ in }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20
        )

        viewModel.searchText = "zuppa"
        await viewModel.loadRecipes(reset: true)
        XCTAssertEqual(viewModel.recipes.map(\.id), [cachedNew.id])

        viewModel.searchText = ""
        await viewModel.loadRecipes(reset: true)

        XCTAssertEqual(viewModel.recipes.map(\.id), [cachedNew.id, cachedOld.id])
        XCTAssertEqual(viewModel.totalCount, 2)
        XCTAssertTrue(viewModel.isUsingLocalCache)
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertNil(viewModel.error)
    }

    func testSortChangeAppliesCachedRecipesWhenSyncFails() async {
        let older = RecipeListTestSupport.makeRecipe(
            title: "Older",
            updatedAt: Date(timeIntervalSince1970: 100)
        )
        let newer = RecipeListTestSupport.makeRecipe(
            title: "Newer",
            updatedAt: Date(timeIntervalSince1970: 200)
        )
        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                listAllTags: { TagsListResponse(tags: []) },
                listRecipes: { _, _, _, _, _ in
                    XCTFail("cacheable browse must not hit listRecipes")
                    return ListRecipesResponse(
                        pagination: PaginationMetadata(limit: 20, offset: 0, total: 0),
                        recipes: []
                    )
                },
                syncRecipes: { _ in throw URLError(.timedOut) }
            ),
            cache: RecipeListCacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in 300 },
                clearSyncCursor: { _ in },
                loadRecipes: { _ in [older, newer] },
                apply: { _, _ in }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20
        )

        viewModel.sortOrder = .newest
        await viewModel.loadRecipes(reset: true)
        XCTAssertEqual(viewModel.recipes.map(\.id), [newer.id, older.id])

        viewModel.sortOrder = .oldest
        await viewModel.loadRecipes(reset: true)
        XCTAssertEqual(viewModel.recipes.map(\.id), [older.id, newer.id])
    }

    func testFilterMatchingNothingWithFailingSyncShowsNoResultsNotError() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Aioli", tags: ["Sauce"])
        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                listAllTags: { TagsListResponse(tags: []) },
                listRecipes: { _, _, _, _, _ in
                    XCTFail("cacheable browse must not hit listRecipes")
                    return ListRecipesResponse(
                        pagination: PaginationMetadata(limit: 20, offset: 0, total: 0),
                        recipes: []
                    )
                },
                syncRecipes: { _ in throw URLError(.timedOut) }
            ),
            cache: RecipeListCacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in 300 },
                clearSyncCursor: { _ in },
                loadRecipes: { _ in [cached] },
                apply: { _, _ in }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20
        )

        viewModel.selectedTags = ["Dessert"]
        await viewModel.loadRecipes(reset: true)

        XCTAssertTrue(viewModel.recipes.isEmpty)
        XCTAssertNil(viewModel.error)
        XCTAssertFalse(viewModel.isLoading)
    }

    func testSyncFailureWithEmptyCacheStillShowsError() async {
        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                listAllTags: { TagsListResponse(tags: []) },
                listRecipes: { limit, offset, _, _, _ in
                    ListRecipesResponse(
                        pagination: PaginationMetadata(limit: limit, offset: offset, total: 0),
                        recipes: []
                    )
                },
                syncRecipes: { _ in throw URLError(.timedOut) }
            ),
            cache: RecipeListCacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in nil },
                clearSyncCursor: { _ in },
                loadRecipes: { _ in [] },
                apply: { _, _ in }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20
        )

        await viewModel.loadRecipes(reset: true)

        XCTAssertNotNil(viewModel.error)
        XCTAssertFalse(viewModel.isLoading)
    }

    private struct ListRequest {
        let limit: Int64
        let offset: Int64
        let query: String?
        let sortBy: SortBy?
        let sortDir: Direction?
    }
}
