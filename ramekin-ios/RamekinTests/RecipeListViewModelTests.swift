import XCTest
@testable import Ramekin

@MainActor
final class RecipeListViewModelTests: XCTestCase {
    func testCacheableBrowseLoadsAndFiltersSyncedCache() async {
        let matching = RecipeListTestSupport.makeRecipe(title: "Dinner", tags: ["Dinner"])
        let hidden = RecipeListTestSupport.makeRecipe(title: "Lunch", tags: ["Lunch"])
        var didSync = false
        var cachedRecipes = [matching, hidden]

        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                syncRecipes: { _, _, _ in
                    didSync = true
                    return RecipeListTestSupport.emptySyncResponse()
                }
            ),
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                loadRecipes: { _ in cachedRecipes },
                apply: { _, _ in cachedRecipes = [matching, hidden] }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        viewModel.selectedTags = ["Dinner"]
        await viewModel.loadRecipes()

        XCTAssertTrue(didSync)
        XCTAssertEqual(viewModel.recipes.map(\.id), [matching.id])
    }

    func testClearFiltersClearsSearchAndPersistedFilters() {
        let viewModel = RecipeListViewModel(
            api: RecipeListTestSupport.emptyAPIClient(),
            cache: RecipeListTestSupport.noCacheClient(),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
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
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                syncRecipes: { _, _, _ in throw URLError(.timedOut) }
            ),
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) },
                loadRecipes: { _ in [cachedOld, cachedNew] }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        viewModel.searchText = "zuppa"
        await viewModel.loadRecipes()
        XCTAssertEqual(viewModel.recipes.map(\.id), [cachedNew.id])

        viewModel.searchText = ""
        await viewModel.loadRecipes()

        XCTAssertEqual(viewModel.recipes.map(\.id), [cachedNew.id, cachedOld.id])
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
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                syncRecipes: { _, _, _ in throw URLError(.timedOut) }
            ),
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) },
                loadRecipes: { _ in [older, newer] }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        viewModel.sortOrder = .newest
        await viewModel.loadRecipes()
        XCTAssertEqual(viewModel.recipes.map(\.id), [newer.id, older.id])

        viewModel.sortOrder = .oldest
        await viewModel.loadRecipes()
        XCTAssertEqual(viewModel.recipes.map(\.id), [older.id, newer.id])
    }

    func testFilterMatchingNothingWithFailingSyncShowsNoResultsNotError() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Aioli", tags: ["Sauce"])
        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                syncRecipes: { _, _, _ in throw URLError(.timedOut) }
            ),
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncPosition: { _ in RecipeSyncPosition(cursor: 300) },
                loadRecipes: { _ in [cached] }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        viewModel.selectedTags = ["Dessert"]
        await viewModel.loadRecipes()

        XCTAssertTrue(viewModel.recipes.isEmpty)
        XCTAssertNil(viewModel.error)
        XCTAssertFalse(viewModel.isLoading)
    }

    func testSyncFailureWithEmptyCacheStillShowsError() async {
        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                syncRecipes: { _, _, _ in throw URLError(.timedOut) }
            ),
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        await viewModel.loadRecipes()

        XCTAssertNotNil(viewModel.error)
        XCTAssertFalse(viewModel.isLoading)
    }
}
