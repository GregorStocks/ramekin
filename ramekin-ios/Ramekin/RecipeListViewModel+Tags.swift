import Foundation

/// Tag filter state: loading the persisted selection, syncing the available
/// tags from the server's tags feed, and keeping both in the per-account cache.
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

    /// A tag was renamed or deleted here: the tags feed and the recipe feed
    /// both carry the change, so a sync of each picks it up.
    func handleTagsDidChange() {
        loadPersistedTags()
        Task { await loadTags() }
        reloadRecipes()
    }

    func loadTags() async {
        guard let accountKey = cache.currentAccountKey() else {
            selectedTags = []
            availableTags = []
            return
        }
        do {
            let tags = try await DebugLogger.shared.timed("syncTags API", source: "RecipeList") {
                try await TagSyncSupport.refresh(accountKey: accountKey, userDefaults: userDefaults, fetch: api.syncTags)
            }
            guard cache.currentAccountKey() == accountKey else { return }
            availableTags = tags
            TagFilterCache.pruneSelectedTags(
                validNames: Set(tags.map(\.name)),
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
