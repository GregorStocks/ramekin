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

    var body: some View {
        NavigationStack {
            Group {
                if isLoading && recipes.isEmpty {
                    ProgressView("Loading recipes...")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
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
                } else {
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
                }
            }
            .searchable(text: $searchText, prompt: "Search recipes")
            .onChange(of: searchText) { _ in
                searchTask?.cancel()
                searchTask = Task {
                    try? await Task.sleep(nanoseconds: 300_000_000)
                    await loadRecipes()
                }
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
            let response = try await RecipesAPI.listRecipes(
                limit: 50,
                offset: 0,
                q: query.isEmpty ? nil : query,
                sortBy: .title,
                sortDir: .asc
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
