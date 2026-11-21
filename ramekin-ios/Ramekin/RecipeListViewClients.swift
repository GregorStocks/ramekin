import Foundation

/// Injection seams for RecipeListViewModel, so tests can drive the list without
/// a network or an on-disk cache.
struct RecipeListViewAPIClient {
    var syncTags: (_ cursor: Int64?) async throws -> SyncTagsResponse
    var syncRecipes: (
        _ cursor: Int64?,
        _ limit: Int64,
        _ afterId: UUID?
    ) async throws -> SyncRecipesResponse

    static let live = RecipeListViewAPIClient(
        syncTags: { try await SyncAPI.syncTags(cursor: $0) },
        syncRecipes: { cursor, limit, afterId in
            try await SyncAPI.syncRecipes(limit: limit, cursor: cursor, afterId: afterId)
        }
    )
}

@MainActor
struct RecipeListCacheClient {
    var currentAccountKey: () -> String?
    var syncPosition: (_ accountKey: String) -> RecipeSyncPosition?
    var loadSearchCorpus: (_ accountKey: String) async throws -> RecipeSearchCorpus
    /// Applies a sync page and records the position it leaves the cache at.
    var apply: (_ syncResponse: SyncRecipesResponse, _ accountKey: String) async throws -> Void

    static let live = RecipeListCacheClient(
        currentAccountKey: { RecipeCacheStore.shared.currentAccountKey() },
        syncPosition: { RecipeCacheStore.shared.syncPosition(accountKey: $0) },
        loadSearchCorpus: { try await RecipeCacheStore.shared.loadSearchCorpus(accountKey: $0) },
        apply: { try await RecipeCacheStore.shared.apply(syncResponse: $0, accountKey: $1) }
    )
}
