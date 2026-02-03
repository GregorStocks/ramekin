import SwiftUI

struct RecipeDetailView: View {
    let recipeId: UUID

    @State private var recipe: RecipeResponse?
    @State private var isLoading = false
    @State private var error: String?
    @State private var showingAddToShoppingList = false

    var body: some View {
        ScrollView {
            if isLoading && recipe == nil {
                ProgressView()
                    .padding(.top, 100)
            } else if let error = error, recipe == nil {
                errorView(message: error)
            } else if let recipe = recipe {
                recipeContent(recipe)
            }
        }
        .navigationTitle(recipe?.title ?? "Recipe")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let recipe = recipe, !recipe.ingredients.isEmpty {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        showingAddToShoppingList = true
                    } label: {
                        Image(systemName: "cart.badge.plus")
                    }
                }
            }
        }
        .sheet(isPresented: $showingAddToShoppingList) {
            if let recipe = recipe {
                AddToShoppingListSheet(recipe: recipe, isPresented: $showingAddToShoppingList)
            }
        }
        .task {
            await loadRecipe()
        }
    }

    // MARK: - Error View

    private func errorView(message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.largeTitle)
                .foregroundColor(.orange)
            Text(message)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
            Button("Retry") {
                Task { await loadRecipe() }
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
        .frame(maxWidth: .infinity)
    }

    // MARK: - Recipe Content

    private func recipeContent(_ recipe: RecipeResponse) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            // Photo carousel
            if !recipe.photoIds.isEmpty {
                PhotoCarouselView(photoIds: recipe.photoIds)
                    .frame(height: 250)
            }

            VStack(alignment: .leading, spacing: 20) {
                // Header
                headerSection(recipe)

                // Tags
                if !recipe.tags.isEmpty {
                    tagsSection(recipe.tags)
                }

                Divider()

                // Ingredients
                if !recipe.ingredients.isEmpty {
                    ingredientsSection(recipe.ingredients)
                    Divider()
                }

                // Instructions
                instructionsSection(recipe.instructions)

                // Notes
                if let notes = recipe.notes, !notes.isEmpty {
                    Divider()
                    notesSection(notes)
                }

                // Source link
                if let sourceUrl = recipe.sourceUrl, let url = URL(string: sourceUrl) {
                    Divider()
                    sourceLinkSection(url: url, name: recipe.sourceName)
                }
            }
            .padding()
        }
    }

    // MARK: - Sections

    private func headerSection(_ recipe: RecipeResponse) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(recipe.title)
                .font(.title)
                .fontWeight(.bold)

            if let description = recipe.description, !description.isEmpty {
                Text(description)
                    .font(.body)
                    .foregroundColor(.secondary)
            }

            // Time info
            let timeChips = [
                ("Prep", recipe.prepTime),
                ("Cook", recipe.cookTime),
                ("Total", recipe.totalTime)
            ].compactMap { label, value -> (String, String)? in
                guard let value = value, !value.isEmpty else { return nil }
                return (label, value)
            }

            if !timeChips.isEmpty {
                HStack(spacing: 16) {
                    ForEach(timeChips, id: \.0) { label, value in
                        VStack(spacing: 2) {
                            Text(label)
                                .font(.caption)
                                .foregroundColor(.secondary)
                            Text(value)
                                .font(.subheadline)
                                .fontWeight(.medium)
                        }
                    }
                }
            }

            if let servings = recipe.servings, !servings.isEmpty {
                Text("Servings: \(servings)")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
        }
    }

    private func tagsSection(_ tags: [String]) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(tags, id: \.self) { tag in
                    Text(tag)
                        .font(.caption)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 4)
                        .background(Color.orange.opacity(0.2))
                        .foregroundColor(.orange)
                        .clipShape(Capsule())
                }
            }
        }
    }

    private func ingredientsSection(_ ingredients: [Ingredient]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Ingredients")
                .font(.title2)
                .fontWeight(.bold)

            // Group by section
            let grouped = groupIngredientsBySection(ingredients)

            ForEach(grouped, id: \.section) { group in
                if let section = group.section {
                    Text(section)
                        .font(.headline)
                        .padding(.top, 8)
                }

                ForEach(Array(group.items.enumerated()), id: \.offset) { _, ingredient in
                    ingredientRow(ingredient)
                }
            }
        }
    }

    private func ingredientRow(_ ingredient: Ingredient) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Circle()
                .fill(Color.orange)
                .frame(width: 6, height: 6)
                .padding(.top, 6)

            VStack(alignment: .leading, spacing: 2) {
                Text(formatIngredient(ingredient))
                    .font(.body)

                if let note = ingredient.note, !note.isEmpty {
                    Text(note)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .italic()
                }
            }
        }
    }

    private func instructionsSection(_ instructions: String) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Instructions")
                .font(.title2)
                .fontWeight(.bold)

            Text(instructions)
                .font(.body)
        }
    }

    private func notesSection(_ notes: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Notes")
                .font(.title3)
                .fontWeight(.bold)

            Text(notes)
                .font(.body)
                .foregroundColor(.secondary)
        }
    }

    private func sourceLinkSection(url: URL, name: String?) -> some View {
        Link(destination: url) {
            HStack {
                Image(systemName: "link")
                Text(name ?? url.host ?? "View Original")
                Spacer()
                Image(systemName: "arrow.up.right.square")
            }
            .foregroundColor(.orange)
        }
    }

    // MARK: - Helpers

    private func groupIngredientsBySection(_ ingredients: [Ingredient]) -> [(section: String?, items: [Ingredient])] {
        var groups: [(section: String?, items: [Ingredient])] = []
        var currentSection: String?
        var currentItems: [Ingredient] = []

        for ingredient in ingredients {
            if ingredient.section != currentSection {
                if !currentItems.isEmpty {
                    groups.append((section: currentSection, items: currentItems))
                }
                currentSection = ingredient.section
                currentItems = [ingredient]
            } else {
                currentItems.append(ingredient)
            }
        }

        if !currentItems.isEmpty {
            groups.append((section: currentSection, items: currentItems))
        }

        return groups
    }

    private func formatIngredient(_ ingredient: Ingredient) -> String {
        var parts: [String] = []

        if let measurement = ingredient.measurements.first {
            if let amount = measurement.amount, !amount.isEmpty {
                parts.append(amount)
            }
            if let unit = measurement.unit, !unit.isEmpty {
                parts.append(unit)
            }
        }

        parts.append(ingredient.item)

        return parts.joined(separator: " ")
    }

    // MARK: - Data Loading

    private func loadRecipe() async {
        isLoading = true
        error = nil

        do {
            let loaded = try await RecipesAPI.getRecipe(id: recipeId)
            await MainActor.run {
                recipe = loaded
                isLoading = false
            }
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
                isLoading = false
            }
        }
    }
}

// MARK: - Photo Carousel

struct PhotoCarouselView: View {
    let photoIds: [UUID]

    var body: some View {
        TabView {
            ForEach(photoIds, id: \.self) { photoId in
                AuthenticatedImage(url: photoURL(for: photoId))
                    .clipped()
            }
        }
        .tabViewStyle(.page)
    }

    private func photoURL(for photoId: UUID) -> URL? {
        guard let baseURL = RamekinAPI.shared.serverURL else { return nil }
        // Use full size for detail view, not thumbnail
        return URL(string: "\(baseURL)/api/photos/\(photoId.uuidString)")
    }
}

#Preview {
    NavigationStack {
        RecipeDetailView(recipeId: UUID())
    }
}
