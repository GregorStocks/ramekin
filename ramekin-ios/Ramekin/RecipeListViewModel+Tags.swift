import Foundation

/// Tag filter state: loading the persisted selection, refreshing the
/// available tags from the server, and keeping both in the per-account cache.
extension RecipeListViewModel {
    func loadPersistedTags() {
        guard let accountKey = cache.currentAccountKey() else {
            selectedTags = []
            return
        }
        selectedTags = TagFilterCache.loadSelectedTags(accountKey: accountKey, userDefaults: userDefaults)
    }

    func loadPersistedAvailableTags() {
        guard let accountKey = cache.currentAccountKey() else {
            availableTags = []
            return
        }
        availableTags = TagFilterCache.loadAvailableTags(accountKey: accountKey, userDefaults: userDefaults)
    }

    func handleTagsDidChange() {
        loadPersistedTags()
        loadPersistedAvailableTags()
        invalidateRecipeCacheSync()
        reloadRecipes()
    }

    func loadTags() async {
        guard let accountKey = cache.currentAccountKey() else {
            selectedTags = []
            availableTags = []
            return
        }
        do {
            let response = try await DebugLogger.shared.timed("listAllTags API", source: "RecipeList") {
                try await api.listAllTags()
            }
            guard cache.currentAccountKey() == accountKey else { return }
            availableTags = response.tags
            TagFilterCache.saveAvailableTags(
                response.tags,
                accountKey: accountKey,
                userDefaults: userDefaults
            )
            TagFilterCache.pruneSelectedTags(
                validNames: Set(response.tags.map(\.name)),
                accountKey: accountKey,
                userDefaults: userDefaults
            )
            selectedTags = TagFilterCache.loadSelectedTags(
                accountKey: accountKey,
                userDefaults: userDefaults
            )
        } catch is CancellationError {
            DebugLogger.shared.log("loadTags cancelled", source: "RecipeList")
        } catch {
            DebugLogger.shared.log("loadTags error: \(error.localizedDescription)", source: "RecipeList")
        }
    }

    func persistSelectedTags() {
        guard let accountKey = cache.currentAccountKey() else { return }
        TagFilterCache.saveSelectedTags(
            selectedTags,
            accountKey: accountKey,
            userDefaults: userDefaults
        )
    }
}
