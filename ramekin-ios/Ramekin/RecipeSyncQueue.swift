import Foundation

/// Runs recipe cache sync sweeps one at a time. Sweeps write the sync
/// cursor and the pending-sweep record, so two at once (start-up plus
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
    /// cursor is read when this sweep's turn comes, so it reflects the
    /// previous sweep. Cancelling the caller ends its wait at once but
    /// never an earlier sweep.
    func run(
        resumeFromCursor: Bool,
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
                cursor: resumeFromCursor ? cache.syncCursor(accountKey) : nil,
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
