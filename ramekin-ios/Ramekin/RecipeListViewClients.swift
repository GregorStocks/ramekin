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
    var syncRecipes: (_ lastSyncAt: Date?) async throws -> SyncRecipesResponse

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
        syncRecipes: { try await RecipesAPI.syncRecipes(lastSyncAt: $0) }
    )
}

@MainActor
struct RecipeListCacheClient {
    var currentAccountKey: () -> String?
    var lastSyncAt: (_ accountKey: String) -> Date?
    var clearLastSyncAt: (_ accountKey: String) -> Void
    var loadRecipes: (_ accountKey: String) throws -> [RecipeSummary]
    var apply: (_ syncResponse: SyncRecipesResponse, _ accountKey: String) throws -> Void

    static let live = RecipeListCacheClient(
        currentAccountKey: { RecipeCacheStore.shared.currentAccountKey() },
        lastSyncAt: { RecipeCacheStore.shared.lastSyncAt(accountKey: $0) },
        clearLastSyncAt: { RecipeCacheStore.shared.clearLastSyncAt(accountKey: $0) },
        loadRecipes: { try RecipeCacheStore.shared.loadRecipes(accountKey: $0) },
        apply: { try RecipeCacheStore.shared.apply(syncResponse: $0, accountKey: $1) }
    )
}
