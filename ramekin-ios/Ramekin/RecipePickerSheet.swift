import SwiftUI

/// The picker's search results. Every load gets a generation token, so only
/// the latest one may publish or clear the spinner: a slow response to an
/// older query can't overwrite a newer one.
@MainActor
final class RecipePickerViewModel: ObservableObject {
    typealias ListRecipes = (_ query: String?) async throws -> [RecipeSummary]

    @Published private(set) var recipes: [RecipeSummary] = []
    @Published private(set) var isLoading = false
    @Published private(set) var error: String?

    private let listRecipes: ListRecipes
    private var generation = UUID()
    private var searchTask: Task<Void, Never>?

    init(listRecipes: @escaping ListRecipes = RecipePickerViewModel.searchCachedRecipes) {
        self.listRecipes = listRecipes
    }

    /// Searches the synced recipe cache, like the recipe list: relevance
    /// when searching, newest-first when browsing. Only a cache that has
    /// nothing yet (no sync has landed) falls back to asking the server.
    nonisolated static func searchCachedRecipes(_ query: String?) async throws -> [RecipeSummary] {
        let store = RecipeCacheStore.shared
        if let accountKey = store.currentAccountKey() {
            let corpus = try await store.loadSearchCorpus(accountKey: accountKey)
            if !corpus.isEmpty {
                return await Task.detached(priority: .userInitiated) {
                    RecipeSearchSupport.execute(
                        corpus: corpus,
                        parsed: RecipeSearchSupport.parse(query ?? ""),
                        sortBy: nil,
                        sortDir: nil
                    )
                }.value
            }
        }
        return try await RecipesAPI.listRecipes(limit: 50, offset: 0, q: query).recipes
    }

    deinit {
        searchTask?.cancel()
    }

    /// Loads for a search-text change right away: the cache answers in
    /// milliseconds, so there is nothing to debounce.
    func searchTextChanged(_ text: String) {
        searchTask?.cancel()
        searchTask = Task { [weak self] in
            await self?.load(searchText: text)
        }
    }

    func cancelSearch() {
        searchTask?.cancel()
        searchTask = nil
    }

    func load(searchText: String) async {
        let generation = UUID()
        self.generation = generation
        isLoading = true
        error = nil

        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let result = try await listRecipes(query.isEmpty ? nil : query)
            guard self.generation == generation else { return }
            recipes = result
        } catch {
            guard self.generation == generation else { return }
            if !CancellationSupport.isCancellation(error) && !Task.isCancelled {
                self.error = "Could not load recipes."
            }
        }
        isLoading = false
    }
}

struct RecipePickerSheet: View {
    let date: Date
    let mealType: MealType
    let onSelect: (RecipeSummary) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var searchText = ""
    @StateObject private var model = RecipePickerViewModel()

    // As in RecipeListView, .searchable must stay attached to a view that never
    // changes structural identity, or the search bar gets torn down whenever a
    // load/error/empty state swaps the hierarchy. Keep the List permanent and
    // overlay the status views.
    var body: some View {
        NavigationStack {
            List(model.recipes) { recipe in
                Button {
                    onSelect(recipe)
                    dismiss()
                } label: {
                    RecipeRowView(recipe: recipe)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(recipe.title)
                .accessibilityHint("Adds to \(mealType.displayLabel)")
            }
            .listStyle(.plain)
            .overlay { statusOverlay }
            .searchable(
                text: $searchText,
                placement: .navigationBarDrawer(displayMode: .always),
                prompt: "Search recipes"
            )
            .onChange(of: searchText) { newValue in
                model.searchTextChanged(newValue)
            }
            .onDisappear {
                model.cancelSearch()
            }
            .navigationTitle("Add \(mealType.displayLabel)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
            .task { await model.load(searchText: searchText) }
        }
    }

    @ViewBuilder
    private var statusOverlay: some View {
        if model.isLoading && model.recipes.isEmpty {
            ProgressView("Loading recipes…")
        } else if let error = model.error, model.recipes.isEmpty {
            errorView(message: error)
        } else if model.recipes.isEmpty {
            VStack(spacing: 16) {
                Image(systemName: "magnifyingglass")
                    .scaledIconFont(size: 48)
                    .accessibilityHidden(true)
                    .foregroundColor(.secondary)
                Text("No recipes found")
                    .font(.title2)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
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
                Task { await model.load(searchText: searchText) }
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

#Preview {
    RecipePickerSheet(date: Date(), mealType: .dinner) { _ in }
}
