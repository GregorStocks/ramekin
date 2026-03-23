import SwiftUI

struct RecipeListView: View {
    @EnvironmentObject var appState: AppState

    @State private var recipes: [RecipeSummary] = []
    @State private var isLoading = false
    @State private var isLoadingMore = false
    @State private var error: String?
    @State private var hasMore = true
    @State private var loadMoreFailed = false
    @State private var searchText = ""
    @State private var totalCount = 0
    @State private var activeQuery: String?
    @State private var searchTask: Task<Void, Never>?

    @AppStorage("recipeSortOrder") private var sortOrder = RecipeSortOrder.newest
    @AppStorage("recipePhotoFilter") private var photoFilter = PhotoFilter.any
    @AppStorage("recipeSourceFilter") private var sourceFilter = ""
    @AppStorage("recipeCreatedAfterFilter") private var createdAfterFilter = ""
    @AppStorage("recipeCreatedBeforeFilter") private var createdBeforeFilter = ""
    @State private var selectedTags: Set<String> = []
    @State private var availableTags: [TagItem] = []
    @State private var showingAdvancedFilters = false

    private let pageSize: Int64 = 20

    private var hasActiveFilters: Bool {
        currentFilterState.hasAnyFilters
    }

    private var currentFilterState: RecipeListFilterState {
        RecipeListFilterState(
            searchText: searchText,
            selectedTags: selectedTags,
            photoFilter: photoFilter,
            source: sourceFilter,
            createdAfter: createdAfterFilter,
            createdBefore: createdBeforeFilter
        )
    }

    var body: some View {
        Group {
            if isLoading && recipes.isEmpty {
                ProgressView("Loading recipes...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let error = error, recipes.isEmpty {
                errorView(message: error)
            } else if recipes.isEmpty && !hasActiveFilters && searchText.isEmpty {
                emptyStateView
            } else {
                VStack(spacing: 0) {
                    filterBar
                    Divider()
                    if recipes.isEmpty {
                        noResultsView
                    } else {
                        recipeList
                    }
                }
            }
        }
        .searchable(text: $searchText, prompt: "Search recipes")
        .onChange(of: searchText) { _ in
            searchTask?.cancel()
            searchTask = Task {
                try? await Task.sleep(nanoseconds: 300_000_000)
                await loadRecipes(reset: true)
            }
        }
        .navigationTitle("Recipes")
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                HStack(spacing: 16) {
                    NavigationLink(value: NavigationDestination.createRecipe) {
                        Image(systemName: "plus")
                    }
                    sortMenu
                    NavigationLink(value: NavigationDestination.settings) {
                        Image(systemName: "gear")
                    }
                }
            }
        }
        .refreshable {
            DebugLogger.shared.log("Pull-to-refresh started", source: "RecipeList")
            await loadTags()
            await loadRecipes(reset: true)
            DebugLogger.shared.log("Pull-to-refresh completed", source: "RecipeList")
        }
        .task {
            loadPersistedTags()
            loadPersistedAvailableTags()
            await loadTags()
            await loadRecipes(reset: true)
        }
        .onReceive(NotificationCenter.default.publisher(for: .tagsDidChange)) { _ in
            loadPersistedTags()
            loadPersistedAvailableTags()
            reloadRecipes()
        }
        .onReceive(NotificationCenter.default.publisher(for: .recipeDeleted)) { _ in
            Task { await loadRecipes(reset: true) }
        }
        .sheet(isPresented: $showingAdvancedFilters) {
            RecipeAdvancedFiltersSheet(
                source: $sourceFilter,
                createdAfter: $createdAfterFilter,
                createdBefore: $createdBeforeFilter
            ) {
                reloadRecipes()
            }
        }
    }

    // MARK: - Sort Menu

    private var sortMenu: some View {
        Menu {
            ForEach(RecipeSortOrder.allCases, id: \.self) { order in
                Button {
                    sortOrder = order
                    reloadRecipes()
                } label: {
                    if sortOrder == order {
                        Label(order.label, systemImage: "checkmark")
                    } else {
                        Text(order.label)
                    }
                }
            }
        } label: {
            Image(systemName: "arrow.up.arrow.down")
        }
    }

    // MARK: - Filter Bar

    private var filterBar: some View {
        RecipeListFilterBar(
            availableTags: availableTags,
            selectedTags: selectedTags,
            photoFilter: photoFilter,
            advancedFilterLabel: RecipeListFilterSupport.advancedFilterLabel(for: currentFilterState) ?? "Filters",
            hasAdvancedFilters: currentFilterState.hasAdvancedFilters,
            hasActiveFilters: hasActiveFilters,
            onSelectPhotoFilter: { filter in
                photoFilter = filter
                reloadRecipes()
            },
            onOpenAdvancedFilters: {
                showingAdvancedFilters = true
            },
            onToggleTag: toggleTag,
            onClearFilters: clearFilters
        )
    }

    // MARK: - Subviews

    private var recipeList: some View {
        List {
            ForEach(recipes) { recipe in
                NavigationLink(value: NavigationDestination.recipe(recipe.id)) {
                    RecipeRowView(recipe: recipe)
                }
                .accessibilityIdentifier("recipe-row-\(recipe.title)")
            }

            if hasMore {
                if loadMoreFailed {
                    HStack {
                        Spacer()
                        VStack(spacing: 8) {
                            Text("Couldn't load more recipes.")
                                .font(.footnote)
                                .foregroundColor(.secondary)
                            Button("Retry") {
                                Task { await loadMore() }
                            }
                        }
                        Spacer()
                    }
                    .listRowSeparator(.hidden)
                } else {
                    HStack {
                        Spacer()
                        ProgressView()
                        Spacer()
                    }
                    .listRowSeparator(.hidden)
                    .onAppear {
                        Task { await loadMore() }
                    }
                }
            }
        }
        .listStyle(.plain)
    }

    private var emptyStateView: some View {
        VStack(spacing: 16) {
            Image(systemName: "book.closed")
                .font(.system(size: 48))
                .foregroundColor(.secondary)
            Text("No recipes yet")
                .font(.title2)
            Text("Tap + to create a recipe, or use the Share button in Safari to import one")
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var noResultsView: some View {
        VStack(spacing: 16) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 48))
                .foregroundColor(.secondary)
            Text("No matching recipes")
                .font(.title2)
            if hasActiveFilters {
                Button("Clear filters") {
                    clearFilters()
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func errorView(message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.largeTitle)
                .foregroundColor(.orange)
            Text(message)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
            Button("Retry") {
                Task {
                    await loadTags()
                    await loadRecipes(reset: true)
                }
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Data Loading & Filter Logic

extension RecipeListView {
    private func buildQuery() -> String? {
        RecipeListFilterSupport.buildQuery(from: currentFilterState)
    }

    private func toggleTag(_ name: String) {
        if selectedTags.contains(name) {
            selectedTags.remove(name)
        } else {
            selectedTags.insert(name)
        }
        persistSelectedTags()
        reloadRecipes()
    }

    private func clearFilters() {
        selectedTags.removeAll()
        photoFilter = .any
        sourceFilter = ""
        createdAfterFilter = ""
        createdBeforeFilter = ""
        persistSelectedTags()
        reloadRecipes()
    }

    private func reloadRecipes() {
        searchTask?.cancel()
        Task { await loadRecipes(reset: true) }
    }

    private func persistSelectedTags() {
        TagFilterCache.saveSelectedTags(selectedTags)
    }

    private func persistAvailableTags() {
        TagFilterCache.saveAvailableTags(availableTags)
    }

    fileprivate func loadPersistedTags() {
        selectedTags = TagFilterCache.loadSelectedTags()
    }

    fileprivate func loadPersistedAvailableTags() {
        availableTags = TagFilterCache.loadAvailableTags()
    }

    fileprivate func loadTags() async {
        do {
            let response = try await DebugLogger.shared.timed("listAllTags API", source: "RecipeList") {
                try await TagsAPI.listAllTags()
            }
            await MainActor.run {
                availableTags = response.tags
                persistAvailableTags()
                TagFilterCache.pruneSelectedTags(validNames: Set(response.tags.map(\.name)))
                selectedTags = TagFilterCache.loadSelectedTags()
            }
        } catch is CancellationError {
            DebugLogger.shared.log("loadTags cancelled", source: "RecipeList")
        } catch {
            DebugLogger.shared.log("loadTags error: \(error.localizedDescription)", source: "RecipeList")
        }
    }

    fileprivate func loadRecipes(reset: Bool) async {
        let logger = DebugLogger.shared
        let queryValue = buildQuery()
        logger.log("loadRecipes called (reset=\(reset), query=\(queryValue ?? "nil"))", source: "RecipeList")

        await MainActor.run {
            if reset {
                hasMore = true
                loadMoreFailed = false
                isLoadingMore = false
                activeQuery = queryValue
            }

            isLoading = true
            error = nil
        }

        do {
            let response = try await logger.timed("listRecipes API", source: "RecipeList") {
                try await RecipesAPI.listRecipes(
                    limit: pageSize,
                    offset: 0,
                    q: queryValue,
                    sortBy: sortOrder.sortBy,
                    sortDir: sortOrder.sortDir
                )
            }

            await MainActor.run {
                guard activeQuery == queryValue else {
                    logger.log("loadRecipes: stale query, discarding results", source: "RecipeList")
                    return
                }
                recipes = response.recipes
                totalCount = Int(response.pagination.total)
                hasMore = recipes.count < totalCount
                isLoading = false
                logger.log("loadRecipes: got \(response.recipes.count) recipes, total \(response.pagination.total)", source: "RecipeList")
            }
        } catch is CancellationError {
            logger.log("loadRecipes: cancelled", source: "RecipeList")
        } catch {
            logger.log("loadRecipes: error - \(error.localizedDescription)", source: "RecipeList")
            await MainActor.run {
                guard activeQuery == queryValue else { return }
                if recipes.isEmpty {
                    self.error = "Could not load recipes. Please try again."
                }
                isLoading = false
            }
        }
    }

    private func loadMore() async {
        guard !isLoading && !isLoadingMore && hasMore else { return }

        await MainActor.run {
            isLoadingMore = true
            loadMoreFailed = false
        }

        do {
            let response = try await RecipesAPI.listRecipes(
                limit: pageSize,
                offset: Int64(recipes.count),
                q: activeQuery,
                sortBy: sortOrder.sortBy,
                sortDir: sortOrder.sortDir
            )

            await MainActor.run {
                recipes.append(contentsOf: response.recipes)
                totalCount = Int(response.pagination.total)
                hasMore = recipes.count < totalCount
                isLoadingMore = false
            }
        } catch is CancellationError {
            // Task was cancelled, not a real error
        } catch {
            await MainActor.run {
                loadMoreFailed = true
                isLoadingMore = false
            }
        }
    }
}
