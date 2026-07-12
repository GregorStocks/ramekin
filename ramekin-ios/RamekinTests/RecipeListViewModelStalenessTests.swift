import XCTest
@testable import Ramekin

/// A slow listRecipes response must never overwrite the list the user is
/// actually looking at. Mirrors ramekin-ui/src/pages/cookbook/recipeRequests.test.ts.
@MainActor
final class RecipeListViewModelStalenessTests: XCTestCase {
    func testSearchChangeDuringLoadMoreDiscardsStaleAppend() async {
        let pastaPage1 = [
            RecipeListTestSupport.makeRecipe(title: "Pasta 1"),
            RecipeListTestSupport.makeRecipe(title: "Pasta 2")
        ]
        let pastaPage2 = [RecipeListTestSupport.makeRecipe(title: "Pasta 3")]
        let cakeResults = [RecipeListTestSupport.makeRecipe(title: "Cake")]
        let appendStarted = RequestGate()
        let releaseAppend = RequestGate()

        let viewModel = makeViewModel(
            listRecipes: { limit, offset, query, _, _ in
                if offset > 0 {
                    await appendStarted.open()
                    await releaseAppend.wait()
                    return ListRecipesResponse(
                        pagination: PaginationMetadata(limit: limit, offset: offset, total: 10),
                        recipes: pastaPage2
                    )
                }
                let isCake = query == "cake"
                return ListRecipesResponse(
                    pagination: PaginationMetadata(limit: limit, offset: offset, total: isCake ? 1 : 10),
                    recipes: isCake ? cakeResults : pastaPage1
                )
            }
        )

        viewModel.searchText = "pasta"
        await viewModel.loadRecipes(reset: true)
        XCTAssertEqual(viewModel.recipes.map(\.id), pastaPage1.map(\.id))
        XCTAssertTrue(viewModel.hasMore)

        let loadMoreTask = Task { await viewModel.loadMore() }
        await appendStarted.wait()

        viewModel.searchText = "cake"
        await viewModel.loadRecipes(reset: true)

        releaseAppend.open()
        await loadMoreTask.value

        XCTAssertEqual(viewModel.recipes.map(\.id), cakeResults.map(\.id))
        XCTAssertEqual(viewModel.totalCount, 1)
        XCTAssertFalse(viewModel.hasMore)
        XCTAssertFalse(viewModel.isLoadingMore)
        XCTAssertFalse(viewModel.loadMoreFailed)
    }

    // A sort change leaves the query string untouched, so the activeQuery-style
    // guard cannot see it — only the request generation can.
    func testSortChangeDuringLoadMoreDiscardsStaleAppend() async {
        let newestPage1 = [
            RecipeListTestSupport.makeRecipe(title: "B"),
            RecipeListTestSupport.makeRecipe(title: "C")
        ]
        let newestPage2 = [RecipeListTestSupport.makeRecipe(title: "D")]
        let titlePage1 = [RecipeListTestSupport.makeRecipe(title: "A")]
        let appendStarted = RequestGate()
        let releaseAppend = RequestGate()

        let viewModel = makeViewModel(
            listRecipes: { limit, offset, _, sortBy, _ in
                if offset > 0 {
                    await appendStarted.open()
                    await releaseAppend.wait()
                    return ListRecipesResponse(
                        pagination: PaginationMetadata(limit: limit, offset: offset, total: 10),
                        recipes: newestPage2
                    )
                }
                let byTitle = sortBy == .title
                return ListRecipesResponse(
                    pagination: PaginationMetadata(limit: limit, offset: offset, total: byTitle ? 1 : 10),
                    recipes: byTitle ? titlePage1 : newestPage1
                )
            }
        )

        // A source filter keeps this off the cacheable-browse path without
        // adding a text query, so sortBy still reaches the API.
        viewModel.sourceFilter = "NYT"
        viewModel.sortOrder = .newest
        await viewModel.loadRecipes(reset: true)
        XCTAssertEqual(viewModel.recipes.map(\.id), newestPage1.map(\.id))

        let loadMoreTask = Task { await viewModel.loadMore() }
        await appendStarted.wait()

        viewModel.sortOrder = .title
        await viewModel.loadRecipes(reset: true)

        releaseAppend.open()
        await loadMoreTask.value

        XCTAssertEqual(viewModel.recipes.map(\.id), titlePage1.map(\.id))
        XCTAssertEqual(viewModel.totalCount, 1)
        XCTAssertFalse(viewModel.hasMore)
        XCTAssertFalse(viewModel.isLoadingMore)
    }

    // Search reloads are debounced, so an append can resolve after searchText
    // changed but before the reset load starts and bumps the generation.
    func testAppendLandingBeforeDebouncedReloadIsDiscarded() async {
        let pastaPage1 = [
            RecipeListTestSupport.makeRecipe(title: "Pasta 1"),
            RecipeListTestSupport.makeRecipe(title: "Pasta 2")
        ]
        let pastaPage2 = [RecipeListTestSupport.makeRecipe(title: "Pasta 3")]
        let appendStarted = RequestGate()
        let releaseAppend = RequestGate()

        let viewModel = makeViewModel(
            listRecipes: { limit, offset, _, _, _ in
                if offset > 0 {
                    await appendStarted.open()
                    await releaseAppend.wait()
                    return ListRecipesResponse(
                        pagination: PaginationMetadata(limit: limit, offset: offset, total: 10),
                        recipes: pastaPage2
                    )
                }
                return ListRecipesResponse(
                    pagination: PaginationMetadata(limit: limit, offset: offset, total: 10),
                    recipes: pastaPage1
                )
            }
        )

        viewModel.searchText = "pasta"
        await viewModel.loadRecipes(reset: true)

        let loadMoreTask = Task { await viewModel.loadMore() }
        await appendStarted.wait()

        // Nothing has bumped the generation yet — only the filter key can
        // reject this append.
        viewModel.searchText = "cake"
        releaseAppend.open()
        await loadMoreTask.value

        XCTAssertEqual(viewModel.recipes.map(\.id), pastaPage1.map(\.id))
    }

    func testLoadMoreDoesNotStartAfterFilterChangeBeforeReload() async {
        let pastaPage1 = [
            RecipeListTestSupport.makeRecipe(title: "Pasta 1"),
            RecipeListTestSupport.makeRecipe(title: "Pasta 2")
        ]
        var listCallCount = 0

        let viewModel = makeViewModel(
            listRecipes: { limit, offset, _, _, _ in
                listCallCount += 1
                return ListRecipesResponse(
                    pagination: PaginationMetadata(limit: limit, offset: offset, total: 10),
                    recipes: pastaPage1
                )
            }
        )

        viewModel.searchText = "pasta"
        await viewModel.loadRecipes(reset: true)
        XCTAssertEqual(listCallCount, 1)
        XCTAssertTrue(viewModel.hasMore)

        viewModel.searchText = "cake"
        await viewModel.loadMore()

        XCTAssertEqual(listCallCount, 1)
        XCTAssertFalse(viewModel.isLoadingMore)
    }

    private func makeViewModel(
        listRecipes: @escaping (
            _ limit: Int64,
            _ offset: Int64,
            _ query: String?,
            _ sortBy: SortBy?,
            _ sortDir: Direction?
        ) async throws -> ListRecipesResponse
    ) -> RecipeListViewModel {
        RecipeListViewModel(
            api: RecipeListViewAPIClient(
                listAllTags: { TagsListResponse(tags: []) },
                listRecipes: listRecipes,
                syncRecipes: { _ in
                    SyncRecipesResponse(deleted: [], recipes: [], syncTimestamp: Date())
                }
            ),
            cache: RecipeListTestSupport.noCacheClient(),
            userDefaults: RecipeListTestSupport.isolatedDefaults(),
            pageSize: 20
        )
    }
}
