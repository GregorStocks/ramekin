import SwiftUI

struct RecipeListView: View {
    @EnvironmentObject var appState: AppState

    @State private var recipes: [RecipeSummary] = []
    @State private var isLoading = false
    @State private var error: String?
    @State private var hasMore = true
    @State private var searchText = ""
    @State private var totalCount = 0

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
        .onChange(of: searchText) { _ in
            Task { await loadRecipes(reset: true) }
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

    private func loadRecipes(reset: Bool) async {
        if reset {
            recipes = []
            hasMore = true
        }

        isLoading = true
        error = nil

        do {
            let response = try await RecipesAPI.listRecipes(
                limit: pageSize,
                offset: 0,
                q: searchText.isEmpty ? nil : searchText
            )

            await MainActor.run {
                recipes = response.recipes
                totalCount = Int(response.pagination.total)
                hasMore = recipes.count < totalCount
                isLoading = false
            }
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
                isLoading = false
            }
        }
    }

    private func loadMore() async {
        guard !isLoading && hasMore else { return }

        isLoading = true

        do {
            let response = try await RecipesAPI.listRecipes(
                limit: pageSize,
                offset: Int64(recipes.count),
                q: searchText.isEmpty ? nil : searchText
            )

            await MainActor.run {
                recipes.append(contentsOf: response.recipes)
                totalCount = Int(response.pagination.total)
                hasMore = recipes.count < totalCount
                isLoading = false
            }
        } catch {
            await MainActor.run {
                isLoading = false
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
