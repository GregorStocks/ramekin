import Foundation

/// Runs recipe cache sync sweeps one at a time. Sweeps move the cache's
/// sync position, so two at once (start-up plus
/// pull-to-refresh, or the recipe list plus the meal-plan picker) would
/// race on them.
@MainActor
final class RecipeSyncQueue {
    /// The queue for the live cache. Everything syncing `RecipeCacheStore.shared`
    /// goes through it.
    static let shared = RecipeSyncQueue()

    /// The most recently started sweep, and everything queued before it.
    private var last: Task<Void, Never>?

    /// Runs one sweep once any sweep already in flight has finished. The
    /// position is read when this sweep's turn comes, so it reflects the
    /// previous sweep. Cancelling the caller ends its wait at once but
    /// never an earlier sweep.
    func run(
        fromStart: Bool,
        accountKey: String,
        pageSize: Int64,
        api: RecipeListViewAPIClient,
        cache: RecipeListCacheClient
    ) async throws {
        let previous = last
        let sweep = Task { @MainActor in
            if let previous { await CancellationSupport.wait(for: previous) }
            try Task.checkCancellation()
            try await RecipeSyncSweep.run(
                fromStart: fromStart,
                accountKey: accountKey,
                pageSize: pageSize,
                api: api,
                cache: cache
            )
        }
        last = Task { await previous?.value; _ = await sweep.result }
        try await withTaskCancellationHandler {
            try await sweep.value
        } onCancel: {
            sweep.cancel()
        }
    }
}
