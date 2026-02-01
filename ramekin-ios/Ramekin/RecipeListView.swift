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

    private let pageSize: Int64 = 20

    var body: some View {
        Group {
            if isLoading && recipes.isEmpty {
                ProgressView("Loading recipes...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let error = error, recipes.isEmpty {
                errorView(message: error)
            } else if recipes.isEmpty {
                emptyStateView
            } else {
                recipeList
            }
        }
        .searchable(text: $searchText, prompt: "Search recipes")
        .onChange(of: searchText) { newValue in
            searchTask?.cancel()
            let trimmed = newValue.trimmingCharacters(in: .whitespacesAndNewlines)
            searchTask = Task {
                try? await Task.sleep(nanoseconds: 300_000_000)
                await loadRecipes(reset: true, query: trimmed.isEmpty ? nil : trimmed)
            }
        }
        .navigationTitle("Recipes")
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                NavigationLink(value: NavigationDestination.settings) {
                    Image(systemName: "gear")
                }
            }
        }
        .refreshable {
            await loadRecipes(reset: true)
        }
        .task {
            await loadRecipes(reset: true)
        }
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

    // MARK: - Data Loading

    private func loadRecipes(reset: Bool, query: String? = nil) async {
        let queryValue = query ?? (searchText.isEmpty ? nil : searchText)

        await MainActor.run {
            if reset {
                recipes = []
                hasMore = true
                loadMoreFailed = false
                isLoadingMore = false
                activeQuery = queryValue
            }

            isLoading = true
            error = nil
        }

        do {
            let response = try await RecipesAPI.listRecipes(
                limit: pageSize,
                offset: 0,
                q: queryValue
            )

            await MainActor.run {
                guard activeQuery == queryValue else { return }
                recipes = response.recipes
                totalCount = Int(response.pagination.total)
                hasMore = recipes.count < totalCount
                isLoading = false
            }
        } catch {
            await MainActor.run {
                guard activeQuery == queryValue else { return }
                self.error = error.localizedDescription
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
                q: activeQuery
            )

            await MainActor.run {
                recipes.append(contentsOf: response.recipes)
                totalCount = Int(response.pagination.total)
                hasMore = recipes.count < totalCount
                isLoadingMore = false
            }
        } catch {
            await MainActor.run {
                loadMoreFailed = true
                isLoadingMore = false
            }
        }
    }
}

// MARK: - Recipe Row View

struct RecipeRowView: View {
    let recipe: RecipeSummary

    var body: some View {
        HStack(spacing: 12) {
            RecipeThumbnail(photoId: recipe.thumbnailPhotoId, size: 60)

            VStack(alignment: .leading, spacing: 4) {
                Text(recipe.title)
                    .font(.headline)
                    .lineLimit(2)

                if let description = recipe.description, !description.isEmpty {
                    Text(description)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .lineLimit(2)
                }

                if !recipe.tags.isEmpty {
                    Text(recipe.tags.joined(separator: ", "))
                        .font(.caption)
                        .foregroundColor(.orange)
                        .lineLimit(1)
                }
            }
        }
        .padding(.vertical, 4)
    }
}

// MARK: - Navigation Destinations

enum NavigationDestination: Hashable {
    case recipe(UUID)
    case settings
}

#Preview {
    NavigationStack {
        RecipeListView()
    }
    .environmentObject(AppState())
}
