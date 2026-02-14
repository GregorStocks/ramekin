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
    @State private var selectedTags: Set<String> = []
    @State private var availableTags: [TagItem] = []

    private let pageSize: Int64 = 20

    private var hasActiveFilters: Bool {
        !selectedTags.isEmpty || photoFilter != .any
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
            await loadTags()
            await loadRecipes(reset: true)
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
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                photoFilterMenu

                ForEach(availableTags) { tag in
                    Button {
                        toggleTag(tag.name)
                    } label: {
                        chipView(
                            text: tag.name,
                            isSelected: selectedTags.contains(tag.name)
                        )
                    }
                    .buttonStyle(.plain)
                }

                if hasActiveFilters {
                    Button {
                        clearFilters()
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
        }
    }

    private var photoFilterMenu: some View {
        Menu {
            ForEach(PhotoFilter.allCases, id: \.self) { filter in
                Button {
                    photoFilter = filter
                    reloadRecipes()
                } label: {
                    if photoFilter == filter {
                        Label(filter.label, systemImage: "checkmark")
                    } else {
                        Text(filter.label)
                    }
                }
            }
        } label: {
            chipView(
                text: photoFilter != .any ? photoFilter.label : nil,
                icon: "camera",
                isSelected: photoFilter != .any
            )
        }
    }

    private func chipView(text: String? = nil, icon: String? = nil, isSelected: Bool) -> some View {
        HStack(spacing: 4) {
            if let icon = icon {
                Image(systemName: icon)
                    .font(.caption)
            }
            if let text = text {
                Text(text)
                    .font(.caption)
                    .fontWeight(.medium)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(isSelected ? Color.orange : Color(.systemGray5))
        .foregroundColor(isSelected ? .white : .primary)
        .clipShape(Capsule())
    }

    // MARK: - Subviews

    private var recipeList: some View {
        List {
            ForEach(recipes) { recipe in
                NavigationLink(value: NavigationDestination.recipe(recipe.id)) {
                    RecipeRowView(recipe: recipe)
                }
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
            Text("Use the Share button in Safari to save recipes")
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
                Task { await loadRecipes(reset: true) }
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Data Loading & Filter Logic

extension RecipeListView {
    private func buildQuery() -> String? {
        var parts: [String] = []

        let trimmed = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            parts.append(trimmed)
        }

        for tag in selectedTags.sorted() {
            if tag.contains(" ") {
                parts.append("tag:\"\(tag)\"")
            } else {
                parts.append("tag:\(tag)")
            }
        }

        switch photoFilter {
        case .any: break
        case .hasPhotos: parts.append("has:photos")
        case .noPhotos: parts.append("no:photos")
        }

        return parts.isEmpty ? nil : parts.joined(separator: " ")
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
        persistSelectedTags()
        reloadRecipes()
    }

    private func reloadRecipes() {
        searchTask?.cancel()
        Task { await loadRecipes(reset: true) }
    }

    private func persistSelectedTags() {
        if let data = try? JSONEncoder().encode(Array(selectedTags)) {
            UserDefaults.standard.set(data, forKey: "recipeSelectedTags")
        }
    }

    fileprivate func loadPersistedTags() {
        if let data = UserDefaults.standard.data(forKey: "recipeSelectedTags"),
           let names = try? JSONDecoder().decode([String].self, from: data) {
            selectedTags = Set(names)
        }
    }

    fileprivate func loadTags() async {
        do {
            let response = try await DebugLogger.shared.timed("listAllTags API", source: "RecipeList") {
                try await TagsAPI.listAllTags()
            }
            await MainActor.run {
                availableTags = response.tags
                let validNames = Set(response.tags.map(\.name))
                let removed = selectedTags.subtracting(validNames)
                if !removed.isEmpty {
                    selectedTags.subtract(removed)
                    persistSelectedTags()
                }
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
