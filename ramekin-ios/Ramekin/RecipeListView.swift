import SwiftUI

struct RecipeListView: View {
    @StateObject private var viewModel: RecipeListViewModel
    @State private var showingNewRecipe = false

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
                if viewModel.reloadFailed {
                    failureBanner("Couldn't update results — these may be out of date")
                    Divider()
                } else if viewModel.syncFailed {
                    failureBanner("Couldn't refresh — showing saved recipes")
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
            ToolbarItemGroup(placement: .navigationBarTrailing) {
                Button {
                    showingNewRecipe = true
                } label: {
                    Image(systemName: "plus")
                }
                .accessibilityLabel("New Recipe")
                sortMenu
                NavigationLink(value: NavigationDestination.settings) {
                    Image(systemName: "gear")
                }
                .accessibilityLabel("Settings")
            }
        }
        // Creating a recipe is a self-contained task, so it's a modal with
        // Cancel/Save rather than a push whose back swipe drops the draft.
        .sheet(isPresented: $showingNewRecipe) {
            NavigationStack {
                RecipeFormView(mode: .create) {
                    Task { await viewModel.refresh() }
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
            ProgressView("Loading recipes…")
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

    private func failureBanner(_ message: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundColor(.orange)
                .accessibilityHidden(true)
            Text(message)
                .font(.footnote)
                .foregroundColor(.secondary)
            Spacer()
            Button("Retry") {
                viewModel.reloadRecipes()
            }
            .font(.footnote.weight(.semibold))
            .frame(minWidth: 44, minHeight: 44)
            .contentShape(Rectangle())
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
                .scaledIconFont(size: 48)
                .accessibilityHidden(true)
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
                .scaledIconFont(size: 48)
                .accessibilityHidden(true)
                .foregroundColor(.secondary)
            Text("No matching recipes")
                .font(.title2)
            if viewModel.hasActiveFilters {
                Button("Clear Filters") {
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
                .accessibilityHidden(true)
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
