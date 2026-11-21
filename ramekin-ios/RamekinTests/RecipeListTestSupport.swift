import Foundation
@testable import Ramekin

/// Shared fixtures for the RecipeListViewModel test classes.
@MainActor
enum RecipeListTestSupport {
    static func makeRecipe(
        title: String,
        tags: [String] = [],
        createdAt: Date = Date(timeIntervalSince1970: 100),
        updatedAt: Date = Date(timeIntervalSince1970: 200)
    ) -> RecipeSummary {
        RecipeSummary(
            createdAt: createdAt,
            description: nil,
            id: UUID(),
            rating: nil,
            tags: tags,
            thumbnailPhotoId: nil,
            title: title,
            updatedAt: updatedAt
        )
    }

    /// Wrap bare summaries as cached search documents, for tests that only
    /// exercise list behavior rather than search-field matching.
    static func makeDocument(_ summary: RecipeSummary) -> CachedRecipeSearchDocument {
        CachedRecipeSearchDocument(
            summary: summary,
            ingredients: [],
            ingredientMatchText: "[]",
            instructions: "",
            notes: nil
        )
    }

    static func cacheClient(
        currentAccountKey: @escaping () -> String? = { nil },
        syncPosition: @escaping (String) -> RecipeSyncPosition? = { _ in nil },
        loadRecipes: @escaping (String) throws -> [RecipeSummary] = { _ in [] },
        apply: @escaping (SyncRecipesResponse, String) throws -> Void = { _, _ in }
    ) -> RecipeListCacheClient {
        RecipeListCacheClient(
            currentAccountKey: currentAccountKey,
            syncPosition: syncPosition,
            loadSearchCorpus: { RecipeSearchCorpus(documents: try loadRecipes($0).map(makeDocument)) },
            apply: apply
        )
    }

    static func noCacheClient() -> RecipeListCacheClient {
        cacheClient()
    }

    static func emptyTagsResponse() -> SyncTagsResponse {
        SyncTagsResponse(changes: [], cursor: 1, deleted: [], fullResync: false)
    }

    static func emptySyncResponse() -> SyncRecipesResponse {
        SyncRecipesResponse(
            changes: [],
            cursor: 1,
            deleted: [],
            fullResync: false,
            hasMore: false,
            normalizationContractVersion: SearchNormalizationSupport.contractVersion
        )
    }

    static func emptyAPIClient() -> RecipeListViewAPIClient {
        RecipeListViewAPIClient(
            syncTags: { _ in RecipeListTestSupport.emptyTagsResponse() },
            syncRecipes: { _, _, _ in emptySyncResponse() }
        )
    }

    static func isolatedDefaults() -> UserDefaults {
        let suiteName = "RecipeListTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}

/// One-shot latch that holds a fake API call open, so a test can interleave
/// other work while a request is in flight.
@MainActor
final class RequestGate {
    private var continuation: CheckedContinuation<Void, Never>?
    private var isOpen = false

    func wait() async {
        guard !isOpen else { return }
        await withCheckedContinuation { self.continuation = $0 }
    }

    func open() {
        guard !isOpen else { return }
        isOpen = true
        continuation?.resume()
        continuation = nil
    }
}
