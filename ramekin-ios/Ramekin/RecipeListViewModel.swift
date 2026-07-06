import Combine
import Foundation

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
    private var activeQuery: String?
    private var searchTask: Task<Void, Never>?

    init(
        api: RecipeListViewAPIClient = .live,
        cache: RecipeListCacheClient = .live,
        userDefaults: UserDefaults = .standard,
        pageSize: Int64 = 20
    ) {
        self.api = api
        self.cache = cache
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
        selectedTags = TagFilterCache.loadSelectedTags()
    }

    func loadPersistedAvailableTags() {
        availableTags = TagFilterCache.loadAvailableTags()
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
        do {
            let response = try await DebugLogger.shared.timed("listAllTags API", source: "RecipeList") {
                try await api.listAllTags()
            }
            availableTags = response.tags
            persistAvailableTags()
            TagFilterCache.pruneSelectedTags(validNames: Set(response.tags.map(\.name)))
            selectedTags = TagFilterCache.loadSelectedTags()
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
            await syncCachedRecipes(queryValue: queryValue)
            return
        }

        if reset {
            hasMore = true
            loadMoreFailed = false
            isLoadingMore = false
            activeQuery = queryValue
            isUsingLocalCache = false
        }

        isLoading = true
        error = nil

        let useRelevance = RecipeListFilterSupport.hasTextQuery(currentFilterState)

        do {
            let response = try await logger.timed("listRecipes API", source: "RecipeList") {
                try await api.listRecipes(
                    pageSize,
                    0,
                    queryValue,
                    useRelevance ? nil : sortOrder.sortBy,
                    useRelevance ? nil : sortOrder.sortDir
                )
            }

            guard activeQuery == queryValue else {
                logger.log("loadRecipes: stale query, discarding results", source: "RecipeList")
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
            guard activeQuery == queryValue else { return }
            if recipes.isEmpty {
                self.error = "Could not load recipes. Please try again."
            }
            isLoading = false
        }
    }

    func loadMore() async {
        guard !isUsingLocalCache && !isLoading && !isLoadingMore && hasMore else { return }

        isLoadingMore = true
        loadMoreFailed = false

        let useRelevance = RecipeListFilterSupport.hasTextQuery(currentFilterState)

        do {
            let response = try await api.listRecipes(
                pageSize,
                Int64(recipes.count),
                activeQuery,
                useRelevance ? nil : sortOrder.sortBy,
                useRelevance ? nil : sortOrder.sortDir
            )

            recipes.append(contentsOf: response.recipes)
            totalCount = Int(response.pagination.total)
            hasMore = recipes.count < totalCount
            isLoadingMore = false
        } catch is CancellationError {
        } catch {
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
        cache.clearLastSyncAt(accountKey)
    }
}

private extension RecipeListViewModel {
    func buildQuery() -> String? {
        RecipeListFilterSupport.buildQuery(from: currentFilterState)
    }

    func persistSelectedTags() {
        TagFilterCache.saveSelectedTags(selectedTags)
    }

    func persistAvailableTags() {
        TagFilterCache.saveAvailableTags(availableTags)
    }

    func syncCachedRecipes(queryValue: String?) async {
        let logger = DebugLogger.shared

        activeQuery = queryValue
        isUsingLocalCache = true
        hasMore = false
        loadMoreFailed = false
        isLoadingMore = false
        isLoading = recipes.isEmpty
        error = nil

        guard let accountKey = cache.currentAccountKey() else {
            await loadRecipes(reset: true, forceNetwork: true)
            return
        }

        do {
            let cachedBeforeSync = try cache.loadRecipes(accountKey)
            let lastSyncAt = cachedBeforeSync.isEmpty ? nil : cache.lastSyncAt(accountKey)
            let response = try await logger.timed("syncRecipeCache API", source: "RecipeList") {
                try await api.syncRecipes(lastSyncAt)
            }
            try cache.apply(response, accountKey)
            let cachedRecipes = try cache.loadRecipes(accountKey)

            guard activeQuery == queryValue else {
                logger.log("syncCachedRecipes: stale query, discarding results", source: "RecipeList")
                return
            }
            applyCachedRecipes(cachedRecipes)
            logger.log("syncCachedRecipes: cache has \(cachedRecipes.count) recipes", source: "RecipeList")
        } catch is CancellationError {
            logger.log("syncCachedRecipes: cancelled", source: "RecipeList")
        } catch {
            logger.log("syncCachedRecipes: error - \(error.localizedDescription)", source: "RecipeList")
            guard activeQuery == queryValue else { return }
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
        activeQuery = buildQuery()
    }

    static let sortOrderKey = "recipeSortOrder"
    static let photoFilterKey = "recipePhotoFilter"
    static let sourceFilterKey = "recipeSourceFilter"
    static let createdAfterFilterKey = "recipeCreatedAfterFilter"
    static let createdBeforeFilterKey = "recipeCreatedBeforeFilter"
    static let photoSizeFilterKey = "recipePhotoSizeFilter"
    static let photoDimensionFilterKey = "recipePhotoDimensionFilter"
}
