import SwiftUI

struct RecipeListView: View {
    @StateObject private var viewModel: RecipeListViewModel

    @MainActor
    init(viewModel: RecipeListViewModel = RecipeListViewModel()) {
        _viewModel = StateObject(wrappedValue: viewModel)
    }

    var body: some View {
        Group {
            if viewModel.isLoading && viewModel.recipes.isEmpty {
                ProgressView("Loading recipes...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let error = viewModel.error, viewModel.recipes.isEmpty {
                errorView(message: error)
            } else if viewModel.recipes.isEmpty
                && !viewModel.hasActiveFilters
                && viewModel.searchText.isEmpty {
                emptyStateView
            } else {
                VStack(spacing: 0) {
                    filterBar
                    Divider()
                    if viewModel.recipes.isEmpty {
                        noResultsView
                    } else {
                        recipeList
                    }
                }
            }
        }
        .searchable(text: $viewModel.searchText, prompt: "Search recipes")
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
