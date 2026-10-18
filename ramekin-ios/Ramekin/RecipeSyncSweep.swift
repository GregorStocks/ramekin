import Foundation

enum RecipeSyncError: LocalizedError {
    /// The server search contract moved (or the app is ahead of the server).
    /// Caching recipes normalized under a different contract would make
    /// local search silently disagree with server search, so the sync fails
    /// instead.
    case normalizationContractMismatch(server: Int, supported: Int)
    /// The server claimed more pages but sent no recipe to resume after.
    case emptyPageWithMore

    var errorDescription: String? {
        switch self {
        case let .normalizationContractMismatch(server, supported):
            return "Server search contract v\(server) is not supported (app supports v\(supported)); update the app."
        case .emptyPageWithMore:
            return "The server returned an empty sync page that claims more pages follow."
        }
    }
}

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
        var fullResync = false
        if let pending = cache.pendingSyncSweep(accountKey), pending.since == cursor {
            afterId = pending.afterId
            sweepWatermark = pending.watermark
            fullResync = pending.fullResync
        }

        while true {
            // Once the server answers with a full resync, the rest of the
            // sweep asks for one outright: the server judges the cursor per
            // page, and a write landing mid-sweep could make it answer later
            // pages with a delta that omits the unchanged recipes left to prune.
            let response = try await api.syncRecipes(fullResync ? nil : cursor, pageSize, afterId)
            guard response.normalizationContractVersion == SearchNormalizationSupport.contractVersion else {
                throw RecipeSyncError.normalizationContractMismatch(
                    server: response.normalizationContractVersion,
                    supported: SearchNormalizationSupport.contractVersion
                )
            }
            if response.fullResync && !fullResync && afterId != nil {
                // The server switched to a full resync mid-sweep (a restore
                // landed between pages). Pruning only settles the pages it
                // sees, so start over from the first page or recipes the
                // restore lost in the ID range already passed would stay.
                fullResync = true
                afterId = nil
                sweepWatermark = nil
                cache.clearPendingSyncSweep(accountKey)
                continue
            }
            try await cache.apply(response, afterId, accountKey)
            fullResync = fullResync || response.fullResync
            let watermark = sweepWatermark ?? response.cursor
            sweepWatermark = watermark

            if response.hasMore {
                guard let lastId = response.changes.last?.id else {
                    throw RecipeSyncError.emptyPageWithMore
                }
                afterId = lastId
                cache.setPendingSyncSweep(
                    PendingSyncSweep(since: cursor, afterId: lastId, watermark: watermark, fullResync: fullResync),
                    accountKey
                )
            } else {
                cache.setSyncCursor(watermark, accountKey)
                cache.clearPendingSyncSweep(accountKey)
                return
            }
        }
    }

    /// Whether a full-resync page, requested with `afterId`, settles `id`:
    /// the server returns every live recipe in ascending ID order, so a page
    /// covers the IDs past `afterId` through its last recipe, and the final
    /// page runs to the end. A cached recipe in that range the page didn't
    /// return is gone from the server. A page claiming more pages but
    /// carrying no recipe settles nothing.
    static func fullResyncPageCovers(_ id: UUID, afterId: UUID?, response: SyncRecipesResponse) -> Bool {
        let key = serverOrderKey(id)
        if let afterId, key <= serverOrderKey(afterId) {
            return false
        }
        guard response.hasMore else {
            return true
        }
        guard let lastId = response.changes.last?.id else {
            return false
        }
        return key <= serverOrderKey(lastId)
    }

    /// The server stores IDs as lowercase hyphenated text and pages in that
    /// text's order.
    private static func serverOrderKey(_ id: UUID) -> String {
        id.uuidString.lowercased()
    }
}
