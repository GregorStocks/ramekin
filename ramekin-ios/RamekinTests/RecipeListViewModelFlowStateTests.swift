import XCTest
@testable import Ramekin

/// Loading and failure flags: cancellation never strands a spinner, the
/// stale flag and the error stay exclusive, and cache sync sweeps never overlap.
@MainActor
final class RecipeListViewModelFlowStateTests: XCTestCase {
    func testKeystrokeShowsCachedResultsWithoutWaitingForTheDebouncedReload() async {
        let pasta = RecipeListTestSupport.makeRecipe(title: "Pasta")
        let soup = RecipeListTestSupport.makeRecipe(title: "Soup")
        var syncCount = 0
        var api = RecipeListTestSupport.emptyAPIClient()
        api.syncRecipes = { _, _, _ in
            syncCount += 1
            return RecipeListTestSupport.emptySyncResponse()
        }
        let viewModel = RecipeListViewModel(
            api: api,
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                loadRecipes: { _ in [pasta, soup] }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        viewModel.searchText = "pas"
        viewModel.handleSearchTextChange("pas")
        // Drop the debounced reload: whatever shows up came from the cache.
        viewModel.cancelSearch()
        for _ in 0..<400 where viewModel.recipes.isEmpty {
            try? await Task.sleep(nanoseconds: 5_000_000)
        }

        XCTAssertEqual(viewModel.recipes.map(\.id), [pasta.id])
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertEqual(syncCount, 0)
    }

    func testReloadOfAnAnsweredEmptySearchShowsNoSpinner() async {
        let soup = RecipeListTestSupport.makeRecipe(title: "Soup")
        let gate = RequestGate()
        var api = RecipeListTestSupport.emptyAPIClient()
        api.syncRecipes = { _, _, _ in
            await gate.wait()
            return RecipeListTestSupport.emptySyncResponse()
        }
        let viewModel = RecipeListViewModel(
            api: api,
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                loadRecipes: { _ in [soup] }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        viewModel.searchText = "zzz"
        await viewModel.loadCachedRecipesForCurrentQuery()
        XCTAssertTrue(viewModel.recipes.isEmpty)
        XCTAssertFalse(viewModel.isLoading)

        // The freshening reload must not flash the first-load spinner over
        // "no results" while its sync is in flight.
        let reload = Task { await viewModel.loadRecipes() }
        await Task.yield()
        XCTAssertFalse(viewModel.isLoading)
        gate.open()
        await reload.value
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertTrue(viewModel.recipes.isEmpty)
    }

    func testCancelledLoadClearsSpinner() async {
        for cancellation in [
            CancellationError() as Error,
            URLError(.cancelled),
            RamekinAPI.APIError.networkError(URLError(.cancelled))
        ] {
            var api = RecipeListTestSupport.emptyAPIClient()
            api.syncRecipes = { _, _, _ in throw cancellation }
            let viewModel = RecipeListViewModel(
                api: api,
                cache: RecipeListTestSupport.cacheClient(currentAccountKey: { "account" }),
                userDefaults: RecipeListTestSupport.isolatedDefaults()
            )

            await viewModel.loadRecipes()

            XCTAssertFalse(viewModel.isLoading, "\(cancellation)")
            XCTAssertNil(viewModel.error, "\(cancellation)")
            XCTAssertFalse(viewModel.syncFailed, "\(cancellation)")
        }
    }

    func testLoadWithoutAnAccountIsAnErrorNotASpinner() async {
        let viewModel = RecipeListViewModel(
            api: RecipeListTestSupport.emptyAPIClient(),
            cache: RecipeListTestSupport.noCacheClient(),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        await viewModel.loadRecipes()

        XCTAssertNotNil(viewModel.error)
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertTrue(viewModel.recipes.isEmpty)
    }

    func testOverlappingSweepsRunOneAtATime() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached")
        let firstSweepStarted = RequestGate()
        let releaseFirstSweep = RequestGate()
        var calls = 0
        var active = 0
        var maxActive = 0
        var cursorsSent: [Int64?] = []
        var persistedCursor: Int64? = 300

        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                syncRecipes: { cursor, _, _ in
                    calls += 1
                    let call = calls
                    active += 1
                    maxActive = max(maxActive, active)
                    cursorsSent.append(cursor)
                    if call == 1 {
                        await firstSweepStarted.open()
                        await releaseFirstSweep.wait()
                    }
                    active -= 1
                    return SyncRecipesResponse(
                        changes: [],
                        cursor: 400 + Int64(call),
                        deleted: [],
                        fullResync: false,
                        hasMore: false,
                        normalizationContractVersion: SearchNormalizationSupport.contractVersion
                    )
                }
            ),
            cache: RecipeListTestSupport.cacheClient(
                currentAccountKey: { "account" },
                syncCursor: { _ in persistedCursor },
                setSyncCursor: { cursor, _ in persistedCursor = cursor },
                loadRecipes: { _ in [cached] }
            ),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        let first = Task { await viewModel.loadRecipes() }
        await firstSweepStarted.wait()
        // Pull-to-refresh while the start-up sweep is still running.
        let second = Task { await viewModel.loadRecipes() }
        for _ in 0..<50 { await Task.yield() }

        XCTAssertEqual(calls, 1, "The second sweep must wait for the first")

        releaseFirstSweep.open()
        await first.value
        await second.value

        XCTAssertEqual(maxActive, 1)
        // The second sweep resumes from the cursor the first one persisted.
        XCTAssertEqual(cursorsSent, [300, 401])
        XCTAssertEqual(persistedCursor, 402)
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertFalse(viewModel.syncFailed)
    }

    func testCancelledQueuedSweepStopsWaitingButKeepsOrder() async throws {
        let firstSweepStarted = RequestGate()
        let releaseFirstSweep = RequestGate()
        var calls = 0
        var active = 0
        var maxActive = 0

        let viewModel = RecipeListViewModel(
            api: RecipeListViewAPIClient(
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                syncRecipes: { _, _, _ in
                    calls += 1
                    let call = calls
                    active += 1
                    maxActive = max(maxActive, active)
                    if call == 1 {
                        await firstSweepStarted.open()
                        await releaseFirstSweep.wait()
                    }
                    active -= 1
                    return SyncRecipesResponse(
                        changes: [],
                        cursor: 400 + Int64(call),
                        deleted: [],
                        fullResync: false,
                        hasMore: false,
                        normalizationContractVersion: SearchNormalizationSupport.contractVersion
                    )
                }
            ),
            cache: RecipeListTestSupport.cacheClient(currentAccountKey: { "account" }),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )

        let first = Task { try await viewModel.runSyncSweep(resumeFromCursor: false, accountKey: "account") }
        await firstSweepStarted.wait()
        let queued = Task { try await viewModel.runSyncSweep(resumeFromCursor: false, accountKey: "account") }
        for _ in 0..<20 { await Task.yield() }
        queued.cancel()

        // The cancelled caller returns while the first sweep is still held.
        let queuedResult = await queued.result
        XCTAssertThrowsError(try queuedResult.get()) { XCTAssertTrue($0 is CancellationError) }
        XCTAssertEqual(calls, 1)

        // A later sweep still waits for the first one.
        let third = Task { try await viewModel.runSyncSweep(resumeFromCursor: false, accountKey: "account") }
        for _ in 0..<50 { await Task.yield() }
        XCTAssertEqual(calls, 1, "Cancelling a queued sweep must not let the next one jump ahead")

        releaseFirstSweep.open()
        try await first.value
        try await third.value
        XCTAssertEqual(calls, 2)
        XCTAssertEqual(maxActive, 1)
    }

    func testCancellationRecognizesWrappedCancels() {
        XCTAssertTrue(CancellationSupport.isCancellation(CancellationError()))
        XCTAssertTrue(CancellationSupport.isCancellation(URLError(.cancelled)))
        XCTAssertTrue(CancellationSupport.isCancellation(RamekinAPI.APIError.networkError(URLError(.cancelled))))
        XCTAssertTrue(CancellationSupport.isCancellation(
            ErrorResponse.error(-1, nil, nil, URLError(.cancelled))
        ))
        XCTAssertFalse(CancellationSupport.isCancellation(URLError(.timedOut)))
        XCTAssertFalse(CancellationSupport.isCancellation(RamekinAPI.APIError.invalidResponse))
    }
}
