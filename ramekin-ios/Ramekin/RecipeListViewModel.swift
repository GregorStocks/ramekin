import Combine
import Foundation

/// Identifies the filter+sort combination a list request belongs to. Sort is
/// part of it because a sort change leaves the query string untouched.
private struct ListRequestKey: Equatable {
    let query: String?
    let sortOrder: RecipeSortOrder
}

@MainActor
final class RecipeListViewModel: ObservableObject {
    @Published var recipes: [RecipeSummary] = []
    @Published var isLoading = false
    @Published var isLoadingMore = false
    @Published var error: String?
    @Published var hasMore = true
    @Published var loadMoreFailed = false
    @Published var searchText = ""
    @Published var totalCount = 0
    @Published var isUsingLocalCache = false
    @Published private(set) var staleness: RecipeListStaleness?
    @Published var selectedTags: Set<String> = []
    @Published var availableTags: [SyncTag] = []
    @Published var showingAdvancedFilters = false

    @Published var sortOrder: RecipeSortOrder {
        didSet { userDefaults.set(sortOrder.rawValue, forKey: Self.sortOrderKey) }
    }
    @Published var photoFilter: PhotoFilter {
        didSet { userDefaults.set(photoFilter.rawValue, forKey: Self.photoFilterKey) }
    }
    @Published var sourceFilter: String {
        didSet { userDefaults.set(sourceFilter, forKey: Self.sourceFilterKey) }
    }
    @Published var createdAfterFilter: String {
        didSet { userDefaults.set(createdAfterFilter, forKey: Self.createdAfterFilterKey) }
    }
    @Published var createdBeforeFilter: String {
        didSet { userDefaults.set(createdBeforeFilter, forKey: Self.createdBeforeFilterKey) }
    }
    @Published var photoSizeFilter: String {
        didSet { userDefaults.set(photoSizeFilter, forKey: Self.photoSizeFilterKey) }
    }
    @Published var photoDimensionFilter: String {
        didSet { userDefaults.set(photoDimensionFilter, forKey: Self.photoDimensionFilterKey) }
    }
    @Published var caloriesMinFilter: String {
        didSet { userDefaults.set(caloriesMinFilter, forKey: Self.caloriesMinFilterKey) }
    }
    @Published var caloriesMaxFilter: String {
        didSet { userDefaults.set(caloriesMaxFilter, forKey: Self.caloriesMaxFilterKey) }
    }

    let api: RecipeListViewAPIClient
    let cache: RecipeListCacheClient
    let userDefaults: UserDefaults
    private let pageSize: Int64
    private let syncPageSize: Int64
    /// Wall-clock budget for freshening a list already served from the cache.
    private let syncBudget: TimeInterval
    /// Filter+sort the visible list was loaded with.
    private var activeKey: ListRequestKey?
    /// Bumped by every reset load. A response may only be applied while its
    /// generation is still current, so a filter or sort change discards both
    /// initial and append requests that were already in flight.
    private var requestGeneration = 0
    private var searchTask: Task<Void, Never>?
    /// The most recently started cache sync sweep. Sweeps write the sync
    /// cursor and the pending-sweep record, so a new one waits for the
    /// previous one to finish instead of running alongside it.
    private var lastSyncSweep: Task<Void, Never>?

    init(
        api: RecipeListViewAPIClient = .live,
        cache: RecipeListCacheClient? = nil,
        userDefaults: UserDefaults = .standard,
        pageSize: Int64 = 20,
        syncPageSize: Int64 = 100,
        syncBudget: TimeInterval = 15
    ) {
        self.api = api
        self.cache = cache ?? .live
        self.userDefaults = userDefaults
        self.pageSize = pageSize
        self.syncPageSize = syncPageSize
        self.syncBudget = syncBudget
        sortOrder = RecipeSortOrder(rawValue: userDefaults.string(forKey: Self.sortOrderKey) ?? "")
            ?? .newest
        photoFilter = PhotoFilter(rawValue: userDefaults.string(forKey: Self.photoFilterKey) ?? "")
            ?? .any
        sourceFilter = userDefaults.string(forKey: Self.sourceFilterKey) ?? ""
        createdAfterFilter = userDefaults.string(forKey: Self.createdAfterFilterKey) ?? ""
        createdBeforeFilter = userDefaults.string(forKey: Self.createdBeforeFilterKey) ?? ""
        photoSizeFilter = userDefaults.string(forKey: Self.photoSizeFilterKey) ?? ""
        photoDimensionFilter = userDefaults.string(forKey: Self.photoDimensionFilterKey) ?? ""
        caloriesMinFilter = userDefaults.string(forKey: Self.caloriesMinFilterKey) ?? ""
        caloriesMaxFilter = userDefaults.string(forKey: Self.caloriesMaxFilterKey) ?? ""
    }

    deinit {
        searchTask?.cancel()
    }
}

extension RecipeListViewModel {
    var syncFailed: Bool { staleness == .syncFailed }
    var reloadFailed: Bool { staleness == .reloadFailed }

    func start() async {
        loadPersistedTags()
        loadPersistedAvailableTags()
        await loadCachedRecipesForCurrentQuery()
        await loadTags()
        await loadRecipes()
    }

    func refresh() async {
        DebugLogger.shared.log("Pull-to-refresh started", source: "RecipeList")
        await loadTags()
        await loadRecipes()
        DebugLogger.shared.log("Pull-to-refresh completed", source: "RecipeList")
    }

    func handleSearchTextChange(_ newValue: String) {
        if newValue.isEmpty {
            clearFilters()
        } else {
            SearchDebounceSupport.replaceTask(&searchTask) { [weak self] in
                await self?.loadRecipes()
            }
        }
    }

    func cancelSearch() {
        SearchDebounceSupport.cancelTask(&searchTask)
    }

    func reloadRecipes() {
        cancelSearch()
        Task { await loadRecipes() }
    }

    func selectPhotoFilter(_ filter: PhotoFilter) {
        photoFilter = filter
        reloadRecipes()
    }

    func toggleTag(_ name: String) {
        if selectedTags.contains(name) {
            selectedTags.remove(name)
        } else {
            selectedTags.insert(name)
        }
        persistSelectedTags()
        reloadRecipes()
    }

    func clearFilters() {
        searchText = ""
        selectedTags.removeAll()
        photoFilter = .any
        sourceFilter = ""
        createdAfterFilter = ""
        createdBeforeFilter = ""
        photoSizeFilter = ""
        photoDimensionFilter = ""
        caloriesMinFilter = ""
        caloriesMaxFilter = ""
        persistSelectedTags()
        reloadRecipes()
    }

    func handleRecipeDeleted() {
        Task { await loadRecipes() }
    }

    /// Reloads the list from its first page. Later pages come from loadMore().
    func loadRecipes(forceNetwork: Bool = false) async {
        let logger = DebugLogger.shared
        let queryValue = buildQuery()
        logger.log("loadRecipes called (query=\(queryValue ?? "nil"))", source: "RecipeList")

        if !forceNetwork,
           RecipeSummaryCacheSupport.canServeFromCache(filterState: currentFilterState, sortOrder: sortOrder) {
            await syncCachedRecipes(key: currentKey())
            return
        }

        let key = ListRequestKey(query: queryValue, sortOrder: sortOrder)
        let generation = beginReset(key: key, usingLocalCache: false)

        let useRelevance = RecipeListFilterSupport.hasTextQuery(currentFilterState)

        do {
            // Every parameter comes from `key`, so the response is guaranteed to
            // describe the filter+sort this request was started for.
            let response = try await logger.timed("listRecipes API", source: "RecipeList") {
                try await api.listRecipes(
                    pageSize,
                    0,
                    key.query,
                    useRelevance ? nil : key.sortOrder.sortBy,
                    useRelevance ? nil : key.sortOrder.sortDir
                )
            }

            // Deliberately no currentKey() check, unlike loadMore(). This response
            // replaces the whole list, so it cannot mix two filters together — it
            // just makes the list fresher while a reload is pending. Rejecting it
            // would strand the list when no reload is coming: the advanced-filters
            // sheet mutates sourceFilter and friends on every keystroke but only
            // reloads when applied.
            guard isCurrentRequest(generation, key) else {
                logger.log("loadRecipes: superseded request, discarding results", source: "RecipeList")
                return
            }
            publish(response.recipes, total: Int(response.pagination.total), usingLocalCache: false)
            logger.log(
                "loadRecipes: got \(response.recipes.count) recipes, total \(response.pagination.total)",
                source: "RecipeList"
            )
        } catch {
            guard isCurrentRequest(generation, key) else { return }
            if CancellationSupport.isCancellation(error) || Task.isCancelled {
                // Nothing newer took over the list, so nothing else would
                // clear the spinner (cancelSearch() on disappear lands here).
                logger.log("loadRecipes: cancelled", source: "RecipeList")
            } else {
                logger.log("loadRecipes: error - \(error.localizedDescription)", source: "RecipeList")
                fail(staleness: .reloadFailed)
            }
            isLoading = false
        }
    }

    func loadMore() async {
        guard !isUsingLocalCache && !isLoading && !isLoadingMore && hasMore else { return }
        // The user can change a filter or sort before its reload starts — search
        // is debounced, and reloadRecipes() defers to a Task. Extending a list
        // the user has already moved on from would splice rows from the previous
        // query into it, so leave the next request to the pending reload.
        guard let key = activeKey, key == currentKey() else { return }

        let generation = requestGeneration

        isLoadingMore = true
        loadMoreFailed = false

        let useRelevance = RecipeListFilterSupport.hasTextQuery(currentFilterState)

        do {
            let response = try await api.listRecipes(
                pageSize,
                Int64(recipes.count),
                key.query,
                useRelevance ? nil : key.sortOrder.sortBy,
                useRelevance ? nil : key.sortOrder.sortDir
            )

            guard appendMayApply(generation, key) else {
                DebugLogger.shared.log("loadMore: superseded request, discarding results", source: "RecipeList")
                releaseAppendSlot(generation)
                return
            }
            publish(recipes + response.recipes, total: Int(response.pagination.total), usingLocalCache: false)
        } catch {
            guard appendMayApply(generation, key),
                  !CancellationSupport.isCancellation(error),
                  !Task.isCancelled
            else {
                releaseAppendSlot(generation)
                return
            }
            loadMoreFailed = true
            isLoadingMore = false
        }
    }

    func loadCachedRecipesForCurrentQuery() async {
        guard RecipeSummaryCacheSupport.canServeFromCache(filterState: currentFilterState, sortOrder: sortOrder),
              let accountKey = cache.currentAccountKey()
        else {
            return
        }

        do {
            let cachedDocuments = try await cache.loadSearchDocuments(accountKey)
            await applyCachedRecipes(cachedDocuments)
        } catch {
            DebugLogger.shared.log("loadCachedRecipes error: \(error.localizedDescription)", source: "RecipeList")
        }
    }
}

private extension RecipeListViewModel {
    func buildQuery() -> String? {
        RecipeListFilterSupport.buildQuery(from: currentFilterState)
    }

    func currentKey() -> ListRequestKey {
        ListRequestKey(query: buildQuery(), sortOrder: sortOrder)
    }

    /// A response may only be applied if nothing superseded its request. The
    /// generation catches reloads that leave the query untouched, such as a
    /// sort change; the key check keeps the existing initial-response guard.
    func isCurrentRequest(_ generation: Int, _ key: ListRequestKey) -> Bool {
        generation == requestGeneration && activeKey == key
    }

    /// An append additionally has to match what the user is asking for right
    /// now: a filter or sort change can land before its reload starts.
    func appendMayApply(_ generation: Int, _ key: ListRequestKey) -> Bool {
        isCurrentRequest(generation, key) && key == currentKey()
    }

    /// A reset load owns the loading flags once it bumps the generation. Until
    /// then a discarded append has to clear its own spinner, or it sticks and
    /// the `!isLoadingMore` guard blocks every later page.
    func releaseAppendSlot(_ generation: Int) {
        guard generation == requestGeneration else { return }
        isLoadingMore = false
    }

    /// Starts a reload of the whole list: bumps the generation so every
    /// older response is discarded, and resets every per-list flag in one
    /// place. Returns the new generation.
    func beginReset(key: ListRequestKey, usingLocalCache: Bool) -> Int {
        requestGeneration += 1
        activeKey = key
        isUsingLocalCache = usingLocalCache
        // The cache holds the whole corpus, so a cache-served list never pages.
        hasMore = !usingLocalCache
        loadMoreFailed = false
        isLoadingMore = false
        staleness = nil
        error = nil
        // A cache-served reload keeps the old rows up until the cache answers.
        isLoading = usingLocalCache ? recipes.isEmpty : true
        return requestGeneration
    }

    /// Shows `newRecipes` as the answer to the active request, clearing every
    /// loading flag and failure that answer supersedes. A failed cache sync
    /// stays flagged: a cache answer is no fresher than the cache.
    func publish(_ newRecipes: [RecipeSummary], total: Int, usingLocalCache: Bool) {
        recipes = newRecipes
        totalCount = total
        hasMore = !usingLocalCache && newRecipes.count < total
        isUsingLocalCache = usingLocalCache
        isLoading = false
        isLoadingMore = false
        loadMoreFailed = false
        error = nil
        if staleness == .reloadFailed {
            staleness = nil
        }
    }

    /// Records a failed reload: the full-screen error when there is nothing
    /// to show, otherwise `newStaleness` over the rows still on screen.
    func fail(staleness newStaleness: RecipeListStaleness) {
        if recipes.isEmpty {
            error = "Could not load recipes. Please try again."
            staleness = nil
        } else {
            error = nil
            staleness = newStaleness
            // The rows answer an older request: paging the new one onto them
            // would splice two queries together. Retry reloads instead.
            hasMore = false
        }
    }

    func syncCachedRecipes(key: ListRequestKey) async {
        let logger = DebugLogger.shared

        let generation = beginReset(key: key, usingLocalCache: true)

        guard let accountKey = cache.currentAccountKey() else {
            await loadRecipes(forceNetwork: true)
            return
        }

        // Once the current filter+sort has been answered from the cache, a
        // sync failure must not replace that answer with the full-screen load
        // error — an empty `recipes` can just mean the filters match nothing.
        var appliedCachedRecipes = false
        do {
            let cachedBeforeSync = try await cache.loadSearchDocuments(accountKey)
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
            let cachedDocuments = try await cache.loadSearchDocuments(accountKey)

            guard isCurrentRequest(generation, key) else {
                logger.log("syncCachedRecipes: superseded request, discarding results", source: "RecipeList")
                return
            }
            await applyCachedRecipes(cachedDocuments)
            logger.log("syncCachedRecipes: cache has \(cachedDocuments.count) recipes", source: "RecipeList")
        } catch {
            guard isCurrentRequest(generation, key) else { return }
            if CancellationSupport.isCancellation(error) || Task.isCancelled {
                logger.log("syncCachedRecipes: cancelled", source: "RecipeList")
            } else {
                logger.log("syncCachedRecipes: error - \(error.localizedDescription)", source: "RecipeList")
                if appliedCachedRecipes {
                    // The user is looking at a cache-served list that could
                    // not be freshened. Never leave that silent: new and
                    // edited recipes would just quietly not show up.
                    error = nil
                    staleness = .syncFailed
                } else {
                    fail(staleness: .syncFailed)
                }
            }
            isLoading = false
        }
    }

    /// Runs one sync sweep once any sweep already in flight has finished:
    /// two at once (start-up plus pull-to-refresh) would race on the
    /// persisted cursor and pending-sweep record. The cursor is read when
    /// this sweep's turn comes, so it reflects the previous sweep.
    /// Cancelling the caller cancels its own sweep, never an earlier one.
    func runSyncSweep(resumeFromCursor: Bool, accountKey: String) async throws {
        let previous = lastSyncSweep
        let sweep = Task { @MainActor [api = self.api, cache = self.cache, pageSize = syncPageSize] in
            await previous?.value
            try Task.checkCancellation()
            try await RecipeSyncSweep.run(
                cursor: resumeFromCursor ? cache.syncCursor(accountKey) : nil,
                accountKey: accountKey,
                pageSize: pageSize,
                api: api,
                cache: cache
            )
        }
        lastSyncSweep = Task { _ = await sweep.result }
        try await withTaskCancellationHandler {
            try await sweep.value
        } onCancel: {
            sweep.cancel()
        }
    }

    /// Filters and ranks the cached corpus off the main thread, then
    /// publishes it unless the filter+sort changed or a newer reset load
    /// started meanwhile. Returns whether it published.
    @discardableResult
    func applyCachedRecipes(_ cachedDocuments: [CachedRecipeSearchDocument]) async -> Bool {
        let key = currentKey()
        let generation = requestGeneration
        let filterState = currentFilterState
        let sortOrder = self.sortOrder
        // The filters can move to a server-only query between the caller's
        // routing check and here; the reload that change schedules owns the
        // list then.
        guard RecipeSummaryCacheSupport.canServeFromCache(filterState: filterState, sortOrder: sortOrder) else {
            return false
        }
        let visibleRecipes = await Task.detached(priority: .userInitiated) {
            RecipeSummaryCacheSupport.visibleRecipes(
                documents: cachedDocuments,
                filterState: filterState,
                sortOrder: sortOrder
            )
        }.value
        // A newer reset with the same filter+sort (pull-to-refresh, a reload
        // after a delete) owns the list; this older corpus must not overwrite it.
        guard key == currentKey(), generation == requestGeneration else {
            DebugLogger.shared.log("applyCachedRecipes: superseded while ranking, discarding", source: "RecipeList")
            return false
        }
        activeKey = key
        publish(visibleRecipes, total: visibleRecipes.count, usingLocalCache: true)
        return true
    }
}
