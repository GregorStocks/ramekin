import Foundation

/// The paged recipe cache sync: sweeps the server's recipe-id space under a
/// fixed cursor, applying each page to the cache as it lands so an
/// interrupted sweep resumes from its pending state instead of re-fetching
/// every page.
enum RecipeSyncSweep {
    /// Runs a sweep to completion. The persisted cursor only advances once
    /// the sweep completes, and it advances to the sweep's *first* page
    /// watermark — a change committed mid-sweep can land in an id range the
    /// sweep already passed, and only the first watermark is low enough to
    /// redeliver it.
    @MainActor
    static func run(
        cursor: Int64?,
        accountKey: String,
        pageSize: Int64,
        api: RecipeListViewAPIClient,
        cache: RecipeListCacheClient
    ) async throws {
        var afterId: UUID?
        var sweepWatermark: Int64?
        if let pending = cache.pendingSyncSweep(accountKey), pending.since == cursor {
            afterId = pending.afterId
            sweepWatermark = pending.watermark
        }

        while true {
            let response = try await api.syncRecipes(cursor, pageSize, afterId)
            try cache.apply(response, accountKey)
            let watermark = sweepWatermark ?? response.cursor
            sweepWatermark = watermark

            if response.hasMore {
                guard let lastId = response.recipes.last?.id else {
                    fatalError("Sync page claims more pages but contains no recipes")
                }
                afterId = lastId
                cache.setPendingSyncSweep(
                    PendingSyncSweep(since: cursor, afterId: lastId, watermark: watermark),
                    accountKey
                )
            } else {
                cache.setSyncCursor(watermark, accountKey)
                cache.clearPendingSyncSweep(accountKey)
                return
            }
        }
    }
}
