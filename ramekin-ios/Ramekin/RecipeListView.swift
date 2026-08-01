import SwiftUI

struct RecipeListView: View {
    @StateObject private var viewModel: RecipeListViewModel

    @MainActor
    init() {
        _viewModel = StateObject(wrappedValue: RecipeListViewModel())
    }

    init(viewModel: RecipeListViewModel) {
        _viewModel = StateObject(wrappedValue: viewModel)
    }

    // The search bar's UISearchController is owned by the view .searchable is
    // attached to, so that view must never change structural identity: swapping
    // it out (e.g. for a full-screen spinner) destroys the search bar. Keep the
    // List permanently in the hierarchy and draw status views as overlays.
    var body: some View {
        VStack(spacing: 0) {
            if showsListChrome {
                if viewModel.syncFailed {
                    syncFailedBanner
                    Divider()
                }
                filterBar
                Divider()
            }
            recipeList
                .overlay { statusOverlay }
        }
        .searchable(
            text: $viewModel.searchText,
            placement: .navigationBarDrawer(displayMode: .always),
            prompt: "Search recipes"
        )
        .onChange(of: viewModel.searchText) { newValue in
            viewModel.handleSearchTextChange(newValue)
        }
        .onDisappear {
            viewModel.cancelSearch()
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
            await viewModel.refresh()
        }
        .task {
            await viewModel.start()
        }
        .onReceive(NotificationCenter.default.publisher(for: .tagsDidChange)) { _ in
            viewModel.handleTagsDidChange()
        }
        .onReceive(NotificationCenter.default.publisher(for: .recipeDeleted)) { _ in
            viewModel.handleRecipeDeleted()
        }
        .sheet(isPresented: $viewModel.showingAdvancedFilters) {
            RecipeAdvancedFiltersSheet(
                source: $viewModel.sourceFilter,
                createdAfter: $viewModel.createdAfterFilter,
                createdBefore: $viewModel.createdBeforeFilter,
                photoSizeFilter: $viewModel.photoSizeFilter,
                photoDimensionFilter: $viewModel.photoDimensionFilter
            ) {
                viewModel.reloadRecipes()
            }
        }
    }

    private var showsListChrome: Bool {
        if viewModel.isLoading && viewModel.recipes.isEmpty {
            return false
        }
        if viewModel.error != nil && viewModel.recipes.isEmpty {
            return false
        }
        if viewModel.recipes.isEmpty
            && !viewModel.hasActiveFilters
            && viewModel.searchText.isEmpty {
            return false
        }
        return true
    }

    @ViewBuilder
    private var statusOverlay: some View {
        if viewModel.isLoading && viewModel.recipes.isEmpty {
            ProgressView("Loading recipes...")
        } else if let error = viewModel.error, viewModel.recipes.isEmpty {
            errorView(message: error)
        } else if viewModel.recipes.isEmpty
            && !viewModel.hasActiveFilters
            && viewModel.searchText.isEmpty {
            emptyStateView
        } else if viewModel.recipes.isEmpty {
            noResultsView
        }
    }

    private var syncFailedBanner: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundColor(.orange)
            Text("Couldn't refresh — showing saved recipes")
                .font(.footnote)
                .foregroundColor(.secondary)
            Spacer()
            Button("Retry") {
                viewModel.reloadRecipes()
            }
            .font(.footnote.weight(.semibold))
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
        .background(Color(.systemYellow).opacity(0.15))
    }

    private var sortMenu: some View {
        RecipeSortMenu(sortOrder: $viewModel.sortOrder) {
            viewModel.reloadRecipes()
        }
    }

    private var filterBar: some View {
        RecipeListFilterBar(
            availableTags: viewModel.availableTags,
            selectedTags: viewModel.selectedTags,
            photoFilter: viewModel.photoFilter,
            advancedFilterLabel: viewModel.advancedFilterLabel,
            hasAdvancedFilters: viewModel.currentFilterState.hasAdvancedFilters,
            hasActiveFilters: viewModel.hasActiveFilters,
            onSelectPhotoFilter: viewModel.selectPhotoFilter,
            onOpenAdvancedFilters: {
                viewModel.showingAdvancedFilters = true
            },
            onToggleTag: viewModel.toggleTag,
            onClearFilters: viewModel.clearFilters
        )
    }

    private var recipeList: some View {
        List {
            ForEach(viewModel.recipes) { recipe in
                NavigationLink(value: NavigationDestination.recipe(recipe.id)) {
                    RecipeRowView(recipe: recipe)
                }
            }

            if viewModel.hasMore {
                if viewModel.loadMoreFailed {
                    HStack {
                        Spacer()
                        VStack(spacing: 8) {
                            Text("Couldn't load more recipes.")
                                .font(.footnote)
                                .foregroundColor(.secondary)
                            Button("Retry") {
                                Task { await viewModel.loadMore() }
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
                        Task { await viewModel.loadMore() }
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
            if viewModel.hasActiveFilters {
                Button("Clear filters") {
                    viewModel.clearFilters()
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
                    await viewModel.loadTags()
                    await viewModel.loadRecipes(reset: true)
                }
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
