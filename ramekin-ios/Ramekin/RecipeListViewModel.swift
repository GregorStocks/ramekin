import Combine
import Foundation

/// Identifies the filter+sort combination a list request belongs to. Sort is
/// part of it because a sort change leaves the query string untouched.
/// The cached corpus ranked for one filter+sort.
struct CachedRanking {
    let corpusVersion: UUID
    let key: ListRequestKey
    let recipes: [RecipeSummary]
}

struct ListRequestKey: Equatable {
    let query: String?
    let sortOrder: RecipeSortOrder
}

@MainActor
final class RecipeListViewModel: ObservableObject {
    @Published var recipes: [RecipeSummary] = []
    @Published var isLoading = false
    @Published var error: String?
    @Published var searchText = ""
    /// The list on screen came from the cache and the sync meant to freshen
    /// it failed, so it may be stale.
    @Published var syncFailed = false
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
    let syncPageSize: Int64
    /// Wall-clock budget for freshening a list already served from the cache.
    let syncBudget: TimeInterval
    /// Filter+sort the visible list was loaded with.
    var activeKey: ListRequestKey?
    /// Bumped by every reload. An answer may only be applied while its
    /// generation is still current.
    var requestGeneration = 0
    private var searchTask: Task<Void, Never>?
    /// Serializes this model's sync sweeps with everything else syncing
    /// the same cache.
    let syncQueue: RecipeSyncQueue
    /// The last ranking of the cached corpus, reused while the cache hands
    /// back the same corpus for the same filter+sort.
    var lastRanking: CachedRanking?

    init(
        api: RecipeListViewAPIClient = .live,
        cache: RecipeListCacheClient? = nil,
        userDefaults: UserDefaults = .standard,
        syncPageSize: Int64 = 100,
        syncBudget: TimeInterval = 15
    ) {
        // The live cache has one queue; an injected cache gets its own.
        syncQueue = cache == nil ? .shared : RecipeSyncQueue()
        self.api = api
        self.cache = cache ?? .live
        self.userDefaults = userDefaults
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
    func start() async {
        loadPersistedTags()
        loadPersistedAvailableTags()
        await loadCachedRecipesForCurrentQuery()
        await loadTags()
        await loadRecipes()
    }

    func refresh() async {
        DebugLogger.shared.log("Pull-to-refresh started", source: "RecipeList")
        lastRanking = nil
        await loadTags()
        await loadRecipes()
        DebugLogger.shared.log("Pull-to-refresh completed", source: "RecipeList")
    }

    func handleSearchTextChange(_ newValue: String) {
        if newValue.isEmpty {
            clearFilters()
        } else {
            // The cache answers in milliseconds, so show its results for
            // this keystroke now; the debounced reload below only freshens
            // them from the server.
            Task { await loadCachedRecipesForCurrentQuery() }
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
        // Asking again is how you get a new random order.
        lastRanking = nil
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

    /// Reloads the list: answers the current filter+sort from the cache,
    /// then freshens the cache from the server and answers again.
    func loadRecipes() async {
        DebugLogger.shared.log("loadRecipes called (query=\(buildQuery() ?? "nil"))", source: "RecipeList")
        await syncCachedRecipes(key: currentKey())
    }
}

extension RecipeListViewModel {
    private func buildQuery() -> String? {
        RecipeListFilterSupport.buildQuery(from: currentFilterState)
    }

    func currentKey() -> ListRequestKey {
        ListRequestKey(query: buildQuery(), sortOrder: sortOrder)
    }

    /// An answer may only be applied if nothing superseded its reload. The
    /// generation catches reloads that leave the query untouched, such as a
    /// sort change.
    func isCurrentRequest(_ generation: Int, _ key: ListRequestKey) -> Bool {
        generation == requestGeneration && activeKey == key
    }

    /// Starts a reload of the whole list: bumps the generation so every
    /// older response is discarded, and resets every per-list flag in one
    /// place. Returns the new generation.
    func beginReset(key: ListRequestKey) -> Int {
        // An empty list the cache already produced for this filter+sort is an
        // answer ("no results"), not a list still waiting for its first load.
        let alreadyAnswered = activeKey == key && !isLoading && error == nil
        requestGeneration += 1
        activeKey = key
        syncFailed = false
        error = nil
        // A reload keeps the old rows up until the cache answers.
        isLoading = recipes.isEmpty && !alreadyAnswered
        return requestGeneration
    }

    /// Shows `newRecipes` as the answer to the active request. A failed
    /// sync stays flagged: the answer is no fresher than the cache.
    func publish(_ newRecipes: [RecipeSummary]) {
        // Re-publishing an identical list would make the List diff every row.
        if recipes != newRecipes {
            recipes = newRecipes
        }
        isLoading = false
        error = nil
    }

    /// The cache-served list on screen could not be freshened. Never leave
    /// that silent: new and edited recipes would just quietly not show up.
    func flagSyncFailureOverCachedList() {
        error = nil
        syncFailed = true
    }

    /// Records a sync that failed before the cache answered: the full-screen
    /// error when there is nothing to show, otherwise the stale banner over
    /// the rows still on screen.
    func failSync() {
        if recipes.isEmpty {
            error = "Could not load recipes. Please try again."
            syncFailed = false
        } else {
            flagSyncFailureOverCachedList()
        }
    }
}
