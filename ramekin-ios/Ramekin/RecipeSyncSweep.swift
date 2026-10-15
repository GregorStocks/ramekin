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
        if let pending = cache.pendingSyncSweep(accountKey), pending.since == cursor {
            afterId = pending.afterId
            sweepWatermark = pending.watermark
        }

        while true {
            let response = try await api.syncRecipes(cursor, pageSize, afterId)
            guard response.normalizationContractVersion == SearchNormalizationSupport.contractVersion else {
                throw RecipeSyncError.normalizationContractMismatch(
                    server: response.normalizationContractVersion,
                    supported: SearchNormalizationSupport.contractVersion
                )
            }
            try await cache.apply(response, afterId, accountKey)
            let watermark = sweepWatermark ?? response.cursor
            sweepWatermark = watermark

            if response.hasMore {
                guard let lastId = response.recipes.last?.id else {
                    throw RecipeSyncError.emptyPageWithMore
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
        guard let lastId = response.recipes.last?.id else {
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
