import XCTest
@testable import Ramekin

/// Loading and failure flags: cancellation never strands a spinner, the
/// staleness reasons stay exclusive, and cache sync sweeps never overlap.
@MainActor
final class RecipeListViewModelFlowStateTests: XCTestCase {
    func testCancelledLoadClearsSpinner() async {
        for cancellation in [
            CancellationError() as Error,
            URLError(.cancelled),
            RamekinAPI.APIError.networkError(URLError(.cancelled))
        ] {
            let viewModel = makeViewModel(listRecipes: { _, _ in throw cancellation })

            await viewModel.loadRecipes()

            XCTAssertFalse(viewModel.isLoading, "\(cancellation)")
            XCTAssertNil(viewModel.error, "\(cancellation)")
            XCTAssertNil(viewModel.staleness, "\(cancellation)")
            XCTAssertFalse(viewModel.hasMore, "\(cancellation)")
        }
    }

    func testReloadFailureOverRowsIsStalenessNotError() async {
        let soup = RecipeListTestSupport.makeRecipe(title: "Soup")
        var fail = false
        let viewModel = makeViewModel(listRecipes: { limit, offset in
            if fail { throw URLError(.timedOut) }
            return ListRecipesResponse(
                pagination: PaginationMetadata(limit: limit, offset: offset, total: 5),
                recipes: [soup]
            )
        })

        await viewModel.loadRecipes()
        XCTAssertTrue(viewModel.hasMore)
        fail = true
        await viewModel.loadRecipes()

        XCTAssertEqual(viewModel.staleness, .reloadFailed)
        XCTAssertNil(viewModel.error)
        XCTAssertFalse(viewModel.isLoading)
        XCTAssertFalse(viewModel.hasMore, "Never page a new query onto the old query's rows")

        fail = false
        await viewModel.loadRecipes()

        XCTAssertNil(viewModel.staleness)
        XCTAssertNil(viewModel.error)
    }

    func testFailedFirstLoadIsErrorWithoutStaleness() async {
        let viewModel = makeViewModel(listRecipes: { _, _ in throw URLError(.timedOut) })

        await viewModel.loadRecipes()

        XCTAssertNotNil(viewModel.error)
        XCTAssertNil(viewModel.staleness)
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
                listRecipes: { _, _, _, _, _ in
                    XCTFail("A cache-served list must not page from the server")
                    throw URLError(.badURL)
                },
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
        XCTAssertNil(viewModel.staleness)
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
                listRecipes: { _, _, _, _, _ in throw URLError(.badURL) },
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

    /// A view model that always goes to the server: no account, so no cache.
    private func makeViewModel(
        listRecipes: @escaping (Int64, Int64) async throws -> ListRecipesResponse
    ) -> RecipeListViewModel {
        RecipeListViewModel(
            api: RecipeListViewAPIClient(
                syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
                listRecipes: { limit, offset, _, _, _ in try await listRecipes(limit, offset) },
                syncRecipes: { _, _, _ in RecipeListTestSupport.emptySyncResponse() }
            ),
            cache: RecipeListTestSupport.noCacheClient(),
            userDefaults: RecipeListTestSupport.isolatedDefaults()
        )
    }
}
