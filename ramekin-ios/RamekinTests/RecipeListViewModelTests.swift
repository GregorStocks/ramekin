import XCTest
@testable import Ramekin

@MainActor
final class RecipeListViewModelTests: XCTestCase {
    func testTextSearchUsesRelevanceSortAndStoresResults() async {
        let recipe = makeRecipe(title: "Pasta")
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
                    SyncRecipesResponse(deleted: [], recipes: [], syncTimestamp: Date())
                }
            ),
            cache: noCacheClient(),
            userDefaults: isolatedDefaults(),
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
        let matching = makeRecipe(title: "Dinner", tags: ["Dinner"])
        let hidden = makeRecipe(title: "Lunch", tags: ["Lunch"])
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
                    return SyncRecipesResponse(deleted: [], recipes: [], syncTimestamp: Date())
                }
            ),
            cache: RecipeListCacheClient(
                currentAccountKey: { "account" },
                lastSyncAt: { _ in nil },
                clearLastSyncAt: { _ in },
                loadRecipes: { _ in cachedRecipes },
                apply: { _, _ in cachedRecipes = [matching, hidden] }
            ),
            userDefaults: isolatedDefaults(),
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
            api: emptyAPIClient(),
            cache: noCacheClient(),
            userDefaults: isolatedDefaults(),
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

    private func makeRecipe(
        title: String,
        tags: [String] = [],
        createdAt: Date = Date(timeIntervalSince1970: 100),
        updatedAt: Date = Date(timeIntervalSince1970: 200)
    ) -> RecipeSummary {
        RecipeSummary(
            createdAt: createdAt,
            description: nil,
            id: UUID(),
            rating: nil,
            tags: tags,
            thumbnailPhotoId: nil,
            title: title,
            updatedAt: updatedAt
        )
    }

    private func noCacheClient() -> RecipeListCacheClient {
        RecipeListCacheClient(
            currentAccountKey: { nil },
            lastSyncAt: { _ in nil },
            clearLastSyncAt: { _ in },
            loadRecipes: { _ in [] },
            apply: { _, _ in }
        )
    }

    private func emptyAPIClient() -> RecipeListViewAPIClient {
        RecipeListViewAPIClient(
            listAllTags: { TagsListResponse(tags: []) },
            listRecipes: { limit, offset, _, _, _ in
                ListRecipesResponse(
                    pagination: PaginationMetadata(limit: limit, offset: offset, total: 0),
                    recipes: []
                )
            },
            syncRecipes: { _ in SyncRecipesResponse(deleted: [], recipes: [], syncTimestamp: Date()) }
        )
    }

    private func isolatedDefaults() -> UserDefaults {
        let suiteName = "RecipeListViewModelTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }

    private struct ListRequest {
        let limit: Int64
        let offset: Int64
        let query: String?
        let sortBy: SortBy?
        let sortDir: Direction?
    }
}
