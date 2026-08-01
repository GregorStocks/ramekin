import SwiftUI

struct RecipePickerSheet: View {
    let date: Date
    let mealType: MealType
    let onSelect: (RecipeSummary) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var searchText = ""
    @State private var recipes: [RecipeSummary] = []
    @State private var isLoading = false
    @State private var error: String?
    @State private var searchTask: Task<Void, Never>?

    // As in RecipeListView, .searchable must stay attached to a view that never
    // changes structural identity, or the search bar gets torn down whenever a
    // load/error/empty state swaps the hierarchy. Keep the List permanent and
    // overlay the status views.
    var body: some View {
        NavigationStack {
            List(recipes) { recipe in
                Button {
                    onSelect(recipe)
                    dismiss()
                } label: {
                    RecipeRowView(recipe: recipe)
                }
                .buttonStyle(.plain)
            }
            .listStyle(.plain)
            .overlay { statusOverlay }
            .searchable(
                text: $searchText,
                placement: .navigationBarDrawer(displayMode: .always),
                prompt: "Search recipes"
            )
            .onChange(of: searchText) { _ in
                SearchDebounceSupport.replaceTask(&searchTask) {
                    await loadRecipes()
                }
            }
            .onDisappear {
                SearchDebounceSupport.cancelTask(&searchTask)
            }
            .navigationTitle("Add \(mealType.displayLabel)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
            .task { await loadRecipes() }
        }
    }

    @ViewBuilder
    private var statusOverlay: some View {
        if isLoading && recipes.isEmpty {
            ProgressView("Loading recipes...")
        } else if let error = error, recipes.isEmpty {
            errorView(message: error)
        } else if recipes.isEmpty {
            VStack(spacing: 16) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 48))
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
                .font(.largeTitle)
                .foregroundColor(.orange)
            Text(message)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
            Button("Retry") {
                Task { await loadRecipes() }
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func loadRecipes() async {
        await MainActor.run {
            isLoading = true
            error = nil
        }

        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)

        do {
            // No explicit sort, matching the web meal-plan picker: the
            // server ranks by relevance when searching, newest-first when
            // browsing.
            let response = try await RecipesAPI.listRecipes(
                limit: 50,
                offset: 0,
                q: query.isEmpty ? nil : query
            )
            await MainActor.run {
                recipes = response.recipes
                isLoading = false
            }
        } catch is CancellationError {
            // Task was cancelled, not a real error
        } catch {
            await MainActor.run {
                self.error = "Could not load recipes."
                isLoading = false
            }
        }
    }
}

#Preview {
    RecipePickerSheet(date: Date(), mealType: .dinner) { _ in }
}
