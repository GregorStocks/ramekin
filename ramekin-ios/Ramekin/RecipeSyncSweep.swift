import Foundation

enum RecipeSyncError: LocalizedError {
    /// The server search contract moved (or the app is ahead of the server).
    /// Caching recipes normalized under a different contract would make
    /// local search silently disagree with server search, so the sync fails
    /// instead.
    case normalizationContractMismatch(server: Int, supported: Int)
    /// The server claimed more pages but didn't say where they start.
    case morePagesWithoutPosition

    var errorDescription: String? {
        switch self {
        case let .normalizationContractMismatch(server, supported):
            return "Server search contract v\(server) is not supported (app supports v\(supported)); update the app."
        case .morePagesWithoutPosition:
            return "The server returned a sync page that claims more follow but not where they start."
        }
    }
}

/// The recipe cache sync: pages through the server's change stream from the
/// cache's position to the end. The first sync is the same thing from the
/// start of the stream. Each page moves the position as it is applied, so an
/// interrupted sync picks up where it stopped.
enum RecipeSyncSweep {
    /// `fromStart` ignores the cache's position, which has the server send
    /// every recipe and the cache drop whatever it held.
    @MainActor
    static func run(
        fromStart: Bool,
        accountKey: String,
        pageSize: Int64,
        api: RecipeListViewAPIClient,
        cache: RecipeListCacheClient
    ) async throws {
        var position = fromStart ? nil : cache.syncPosition(accountKey)
        while true {
            let response = try await api.syncRecipes(position?.cursor, pageSize, position?.afterId)
            guard response.normalizationContractVersion == SearchNormalizationSupport.contractVersion else {
                throw RecipeSyncError.normalizationContractMismatch(
                    server: response.normalizationContractVersion,
                    supported: SearchNormalizationSupport.contractVersion
                )
            }
            if response.hasMore && response.afterId == nil {
                throw RecipeSyncError.morePagesWithoutPosition
            }
            try await cache.apply(response, accountKey)
            guard response.hasMore else { return }
            position = RecipeSyncPosition(cursor: response.cursor, afterId: response.afterId)
        }
    }
}
