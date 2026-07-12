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

    static func noCacheClient() -> RecipeListCacheClient {
        RecipeListCacheClient(
            currentAccountKey: { nil },
            lastSyncAt: { _ in nil },
            clearLastSyncAt: { _ in },
            loadRecipes: { _ in [] },
            apply: { _, _ in }
        )
    }

    static func emptyAPIClient() -> RecipeListViewAPIClient {
        RecipeListViewAPIClient(
            listAllTags: { TagsListResponse(tags: []) },
            listRecipes: { limit, offset, _, _, _ in
                ListRecipesResponse(
                    pagination: PaginationMetadata(limit: limit, offset: offset, total: 0),
                    recipes: []
                )
            },
            syncRecipes: { _ in SyncRecipesResponse(deleted: [], recipes: [], syncTimestamp: Date()) }
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
