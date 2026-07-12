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
    @Published var selectedTags: Set<String> = []
    @Published var availableTags: [TagItem] = []
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

    private let api: RecipeListViewAPIClient
    private let cache: RecipeListCacheClient
    private let userDefaults: UserDefaults
    private let pageSize: Int64
    /// Filter+sort the visible list was loaded with.
    private var activeKey: ListRequestKey?
    /// Bumped by every reset load. A response may only be applied while its
    /// generation is still current, so a filter or sort change discards both
    /// initial and append requests that were already in flight.
    private var requestGeneration = 0
    private var searchTask: Task<Void, Never>?

    init(
        api: RecipeListViewAPIClient = .live,
        cache: RecipeListCacheClient? = nil,
        userDefaults: UserDefaults = .standard,
        pageSize: Int64 = 20
    ) {
        self.api = api
        self.cache = cache ?? .live
        self.userDefaults = userDefaults
        self.pageSize = pageSize
        sortOrder = RecipeSortOrder(rawValue: userDefaults.string(forKey: Self.sortOrderKey) ?? "")
            ?? .newest
        photoFilter = PhotoFilter(rawValue: userDefaults.string(forKey: Self.photoFilterKey) ?? "")
            ?? .any
        sourceFilter = userDefaults.string(forKey: Self.sourceFilterKey) ?? ""
        createdAfterFilter = userDefaults.string(forKey: Self.createdAfterFilterKey) ?? ""
        createdBeforeFilter = userDefaults.string(forKey: Self.createdBeforeFilterKey) ?? ""
        photoSizeFilter = userDefaults.string(forKey: Self.photoSizeFilterKey) ?? ""
        photoDimensionFilter = userDefaults.string(forKey: Self.photoDimensionFilterKey) ?? ""
    }

    deinit {
        searchTask?.cancel()
    }
}

extension RecipeListViewModel {
    var hasActiveFilters: Bool {
        currentFilterState.hasAnyFilters
    }

    var currentFilterState: RecipeListFilterState {
        RecipeListFilterState(
            searchText: searchText,
            selectedTags: selectedTags,
            photoFilter: photoFilter,
            source: sourceFilter,
            createdAfter: createdAfterFilter,
            createdBefore: createdBeforeFilter,
            photoSizeThreshold: RecipeListFilterSupport.numericThreshold(from: photoSizeFilter),
            photoDimensionThreshold: RecipeListFilterSupport.numericThreshold(from: photoDimensionFilter)
        )
    }

    var advancedFilterLabel: String {
        RecipeListFilterSupport.advancedFilterLabel(for: currentFilterState) ?? "Filters"
    }

    func start() async {
        loadPersistedTags()
        loadPersistedAvailableTags()
        loadCachedRecipesForCurrentQuery()
        await loadTags()
        await loadRecipes(reset: true)
    }

    func refresh() async {
        DebugLogger.shared.log("Pull-to-refresh started", source: "RecipeList")
        await loadTags()
        await loadRecipes(reset: true)
        DebugLogger.shared.log("Pull-to-refresh completed", source: "RecipeList")
    }

    func handleSearchTextChange(_ newValue: String) {
        if newValue.isEmpty {
            clearFilters()
        } else {
            SearchDebounceSupport.replaceTask(&searchTask) { [weak self] in
                await self?.loadRecipes(reset: true)
            }
        }
    }

    func cancelSearch() {
        SearchDebounceSupport.cancelTask(&searchTask)
    }

    func reloadRecipes() {
        cancelSearch()
        Task { await loadRecipes(reset: true) }
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
        persistSelectedTags()
        reloadRecipes()
    }

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

    func handleRecipeDeleted() {
        Task { await loadRecipes(reset: true) }
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

    func loadRecipes(reset: Bool, forceNetwork: Bool = false) async {
        let logger = DebugLogger.shared
        let queryValue = buildQuery()
        logger.log("loadRecipes called (reset=\(reset), query=\(queryValue ?? "nil"))", source: "RecipeList")

        if !forceNetwork,
           reset,
           RecipeSummaryCacheSupport.canServeFromCache(filterState: currentFilterState, sortOrder: sortOrder) {
            await syncCachedRecipes(key: currentKey())
            return
        }

        if reset {
            hasMore = true
            loadMoreFailed = false
            isLoadingMore = false
            isUsingLocalCache = false
            requestGeneration += 1
        }
        let key = ListRequestKey(query: queryValue, sortOrder: sortOrder)
        activeKey = key
        let generation = requestGeneration

        isLoading = true
        error = nil

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
            recipes = response.recipes
            totalCount = Int(response.pagination.total)
            hasMore = recipes.count < totalCount
            isLoading = false
            logger.log(
                "loadRecipes: got \(response.recipes.count) recipes, total \(response.pagination.total)",
                source: "RecipeList"
            )
        } catch is CancellationError {
            logger.log("loadRecipes: cancelled", source: "RecipeList")
        } catch {
            logger.log("loadRecipes: error - \(error.localizedDescription)", source: "RecipeList")
            guard isCurrentRequest(generation, key) else { return }
            if recipes.isEmpty {
                self.error = "Could not load recipes. Please try again."
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
            recipes.append(contentsOf: response.recipes)
            totalCount = Int(response.pagination.total)
            hasMore = recipes.count < totalCount
            isLoadingMore = false
        } catch is CancellationError {
            releaseAppendSlot(generation)
        } catch {
            guard appendMayApply(generation, key) else {
                releaseAppendSlot(generation)
                return
            }
            loadMoreFailed = true
            isLoadingMore = false
        }
    }

    func loadCachedRecipesForCurrentQuery() {
        guard RecipeSummaryCacheSupport.canServeFromCache(filterState: currentFilterState, sortOrder: sortOrder),
              let accountKey = cache.currentAccountKey()
        else {
            return
        }

        do {
            let cachedRecipes = try cache.loadRecipes(accountKey)
            applyCachedRecipes(cachedRecipes)
        } catch {
            DebugLogger.shared.log("loadCachedRecipes error: \(error.localizedDescription)", source: "RecipeList")
        }
    }

    func invalidateRecipeCacheSync() {
        guard let accountKey = cache.currentAccountKey() else { return }
        cache.clearSyncCursor(accountKey)
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

    func persistSelectedTags() {
        guard let accountKey = cache.currentAccountKey() else { return }
        TagFilterCache.saveSelectedTags(
            selectedTags,
            accountKey: accountKey,
            userDefaults: userDefaults
        )
    }

    func persistAvailableTags() {
        guard let accountKey = cache.currentAccountKey() else { return }
        TagFilterCache.saveAvailableTags(
            availableTags,
            accountKey: accountKey,
            userDefaults: userDefaults
        )
    }

    func syncCachedRecipes(key: ListRequestKey) async {
        let logger = DebugLogger.shared

        activeKey = key
        isUsingLocalCache = true
        hasMore = false
        loadMoreFailed = false
        isLoadingMore = false
        isLoading = recipes.isEmpty
        error = nil
        requestGeneration += 1
        let generation = requestGeneration

        guard let accountKey = cache.currentAccountKey() else {
            await loadRecipes(reset: true, forceNetwork: true)
            return
        }

        do {
            let cachedBeforeSync = try cache.loadRecipes(accountKey)
            let cursor = cachedBeforeSync.isEmpty ? nil : cache.syncCursor(accountKey)
            let response = try await logger.timed("syncRecipeCache API", source: "RecipeList") {
                try await api.syncRecipes(cursor)
            }
            try cache.apply(response, accountKey)
            let cachedRecipes = try cache.loadRecipes(accountKey)

            guard isCurrentRequest(generation, key) else {
                logger.log("syncCachedRecipes: superseded request, discarding results", source: "RecipeList")
                return
            }
            applyCachedRecipes(cachedRecipes)
            logger.log("syncCachedRecipes: cache has \(cachedRecipes.count) recipes", source: "RecipeList")
        } catch is CancellationError {
            logger.log("syncCachedRecipes: cancelled", source: "RecipeList")
        } catch {
            logger.log("syncCachedRecipes: error - \(error.localizedDescription)", source: "RecipeList")
            guard isCurrentRequest(generation, key) else { return }
            if recipes.isEmpty {
                self.error = "Could not load recipes. Please try again."
            }
            isLoading = false
        }
    }

    func applyCachedRecipes(_ cachedRecipes: [RecipeSummary]) {
        let visibleRecipes = RecipeSummaryCacheSupport.filteredAndSorted(
            cachedRecipes,
            filterState: currentFilterState,
            sortOrder: sortOrder
        )
        recipes = visibleRecipes
        totalCount = visibleRecipes.count
        hasMore = false
        isLoading = false
        isLoadingMore = false
        loadMoreFailed = false
        isUsingLocalCache = true
        activeKey = currentKey()
    }

    static let sortOrderKey = "recipeSortOrder"
    static let photoFilterKey = "recipePhotoFilter"
    static let sourceFilterKey = "recipeSourceFilter"
    static let createdAfterFilterKey = "recipeCreatedAfterFilter"
    static let createdBeforeFilterKey = "recipeCreatedBeforeFilter"
    static let photoSizeFilterKey = "recipePhotoSizeFilter"
    static let photoDimensionFilterKey = "recipePhotoDimensionFilter"
}
