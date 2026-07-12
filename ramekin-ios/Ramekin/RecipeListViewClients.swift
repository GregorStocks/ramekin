import Foundation

/// Injection seams for RecipeListViewModel, so tests can drive the list without
/// a network or an on-disk cache.
struct RecipeListViewAPIClient {
    var listAllTags: () async throws -> TagsListResponse
    var listRecipes: (
        _ limit: Int64,
        _ offset: Int64,
        _ query: String?,
        _ sortBy: SortBy?,
        _ sortDir: Direction?
    ) async throws -> ListRecipesResponse
    var syncRecipes: (
        _ cursor: Int64?,
        _ limit: Int64,
        _ afterId: UUID?
    ) async throws -> SyncRecipesResponse

    static let live = RecipeListViewAPIClient(
        listAllTags: { try await TagsAPI.listAllTags() },
        listRecipes: { limit, offset, query, sortBy, sortDir in
            try await RecipesAPI.listRecipes(
                limit: limit,
                offset: offset,
                q: query,
                sortBy: sortBy,
                sortDir: sortDir
            )
        },
        syncRecipes: { cursor, limit, afterId in
            try await RecipesAPI.syncRecipes(limit: limit, cursor: cursor, afterId: afterId)
        }
    )
}

@MainActor
struct RecipeListCacheClient {
    var currentAccountKey: () -> String?
    var syncCursor: (_ accountKey: String) -> Int64?
    var setSyncCursor: (_ cursor: Int64, _ accountKey: String) -> Void
    var clearSyncCursor: (_ accountKey: String) -> Void
    var pendingSyncSweep: (_ accountKey: String) -> PendingSyncSweep?
    var setPendingSyncSweep: (_ sweep: PendingSyncSweep, _ accountKey: String) -> Void
    var clearPendingSyncSweep: (_ accountKey: String) -> Void
    var loadSearchDocuments: (_ accountKey: String) throws -> [CachedRecipeSearchDocument]
    var apply: (_ syncResponse: SyncRecipesResponse, _ accountKey: String) throws -> Void

    static let live = RecipeListCacheClient(
        currentAccountKey: { RecipeCacheStore.shared.currentAccountKey() },
        syncCursor: { RecipeCacheStore.shared.syncCursor(accountKey: $0) },
        setSyncCursor: { RecipeCacheStore.shared.setSyncCursor($0, accountKey: $1) },
        clearSyncCursor: { RecipeCacheStore.shared.clearSyncCursor(accountKey: $0) },
        pendingSyncSweep: { RecipeCacheStore.shared.pendingSyncSweep(accountKey: $0) },
        setPendingSyncSweep: { RecipeCacheStore.shared.setPendingSyncSweep($0, accountKey: $1) },
        clearPendingSyncSweep: { RecipeCacheStore.shared.clearPendingSyncSweep(accountKey: $0) },
        loadSearchDocuments: { try RecipeCacheStore.shared.loadSearchDocuments(accountKey: $0) },
        apply: { try RecipeCacheStore.shared.apply(syncResponse: $0, accountKey: $1) }
    )
}
