import Foundation

/// Serving the list from the local recipe cache, and keeping that cache
/// fresh.
extension RecipeListViewModel {
    /// Shows the cache's answer to the current filter+sort without touching
    /// the network. A cache with no recipes answers nothing: it is either
    /// still waiting for its first sync or has nothing to search.
    func loadCachedRecipesForCurrentQuery() async {
        guard let accountKey = cache.currentAccountKey() else {
            return
        }

        do {
            let corpus = try await cache.loadSearchCorpus(accountKey)
            if !corpus.isEmpty {
                await applyCachedRecipes(corpus)
            }
        } catch {
            DebugLogger.shared.log("loadCachedRecipes error: \(error.localizedDescription)", source: "RecipeList")
        }
    }

    /// Runs one sync sweep, after any already in flight on the same queue.
    func runSyncSweep(resumeFromCursor: Bool, accountKey: String) async throws {
        try await syncQueue.run(
            resumeFromCursor: resumeFromCursor,
            accountKey: accountKey,
            pageSize: syncPageSize,
            api: api,
            cache: cache
        )
    }

    func syncCachedRecipes(key: ListRequestKey) async {
        let logger = DebugLogger.shared

        let generation = beginReset(key: key)

        guard let accountKey = cache.currentAccountKey() else {
            // No signed-in account, so nothing to sync or show.
            logger.log("syncCachedRecipes: no account", source: "RecipeList")
            failSync()
            isLoading = false
            return
        }

        // Once the current filter+sort has been answered from the cache, a
        // sync failure must not replace that answer with the full-screen load
        // error — an empty `recipes` can just mean the filters match nothing.
        var appliedCachedRecipes = false
        do {
            let cachedBeforeSync = try await cache.loadSearchCorpus(accountKey)
            // Sorting, tag filters, and clearing a search are answerable from
            // the cache alone, so apply them before the network round-trip.
            // The sync below only freshens the data: if it fails or hangs, the
            // list must not stay stuck on the previous filter+sort.
            if !cachedBeforeSync.isEmpty {
                appliedCachedRecipes = await applyCachedRecipes(cachedBeforeSync)
            }
            let resumeFromCursor = !cachedBeforeSync.isEmpty
            try await logger.timed("syncRecipeCache API", source: "RecipeList") {
                // When the list was already served from the cache, the sweep
                // only freshens it — give it a wall-clock budget so awaiting
                // callers like pull-to-refresh resolve promptly and a slow
                // sync surfaces as the stale banner instead of a hang.
                if appliedCachedRecipes {
                    try await withWallClockBudget(seconds: syncBudget) { [self] in
                        try await runSyncSweep(resumeFromCursor: resumeFromCursor, accountKey: accountKey)
                    }
                } else {
                    try await runSyncSweep(resumeFromCursor: resumeFromCursor, accountKey: accountKey)
                }
            }
            let corpus = try await cache.loadSearchCorpus(accountKey)

            guard isCurrentRequest(generation, key) else {
                logger.log("syncCachedRecipes: superseded request, discarding results", source: "RecipeList")
                return
            }
            await applyCachedRecipes(corpus)
            logger.log("syncCachedRecipes: cache has \(corpus.entries.count) recipes", source: "RecipeList")
        } catch {
            guard isCurrentRequest(generation, key) else { return }
            if CancellationSupport.isCancellation(error) || Task.isCancelled {
                logger.log("syncCachedRecipes: cancelled", source: "RecipeList")
            } else {
                logger.log("syncCachedRecipes: error - \(error.localizedDescription)", source: "RecipeList")
                if appliedCachedRecipes {
                    flagSyncFailureOverCachedList()
                } else {
                    failSync()
                }
            }
            isLoading = false
        }
    }

    /// Filters and ranks the cached corpus off the main thread, then
    /// publishes it unless the filter+sort changed or a newer reset load
    /// started meanwhile. Returns whether it published.
    @discardableResult
    func applyCachedRecipes(_ corpus: RecipeSearchCorpus) async -> Bool {
        let key = currentKey()
        let generation = requestGeneration
        let filterState = currentFilterState
        let sortOrder = self.sortOrder
        let visibleRecipes: [RecipeSummary]
        if let lastRanking, lastRanking.corpusVersion == corpus.version, lastRanking.key == key {
            // A sync that changed nothing hands back the corpus this
            // filter+sort was already ranked against.
            visibleRecipes = lastRanking.recipes
        } else {
            visibleRecipes = await Task.detached(priority: .userInitiated) {
                RecipeSummaryCacheSupport.visibleRecipes(
                    corpus: corpus,
                    filterState: filterState,
                    sortOrder: sortOrder
                )
            }.value
            lastRanking = CachedRanking(corpusVersion: corpus.version, key: key, recipes: visibleRecipes)
        }
        // A newer reset with the same filter+sort (pull-to-refresh, a reload
        // after a delete) owns the list; this older corpus must not overwrite it.
        guard key == currentKey(), generation == requestGeneration else {
            DebugLogger.shared.log("applyCachedRecipes: superseded while ranking, discarding", source: "RecipeList")
            return false
        }
        activeKey = key
        publish(visibleRecipes)
        return true
    }
}
