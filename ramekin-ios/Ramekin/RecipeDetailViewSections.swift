import SwiftUI

extension RecipeDetailView {
    func recipeContent(_ recipe: RecipeResponse) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if !recipe.photoIds.isEmpty {
                PhotoCarouselView(photoIds: recipe.photoIds)
                    .frame(height: 250)
            }

            VStack(alignment: .leading, spacing: 20) {
                if let error {
                    inlineErrorBanner(message: error)
                }

                headerSection(recipe)

                if isViewingHistoricalVersion {
                    historicalVersionBanner(recipe)
                }

                versionHistorySection

                if !recipe.tags.isEmpty {
                    tagsSection(recipe.tags)
                }

                Divider()

                if !recipe.ingredients.isEmpty {
                    ingredientsSection(recipe.ingredients)
                    Divider()
                }

                instructionsSection(recipe.instructions)

                if let notes = recipe.notes, !notes.isEmpty {
                    Divider()
                    notesSection(notes)
                }

                if let sourceUrl = recipe.sourceUrl, let url = URL(string: sourceUrl) {
                    Divider()
                    sourceLinkSection(url: url, name: recipe.sourceName)
                }
            }
            .padding()
        }
    }

    func headerSection(_ recipe: RecipeResponse) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(recipe.title)
                .font(.title)
                .fontWeight(.bold)

            if let description = recipe.description, !description.isEmpty {
                Text(description)
                    .font(.body)
                    .foregroundColor(.secondary)
            }

            let timeChips = [
                ("Prep", recipe.prepTime),
                ("Cook", recipe.cookTime),
                ("Total", recipe.totalTime)
            ].compactMap { label, value -> (String, String)? in
                guard let value, !value.isEmpty else { return nil }
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

    func tagsSection(_ tags: [String]) -> some View {
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

    func ingredientsSection(_ ingredients: [Ingredient]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Ingredients")
                .font(.title2)
                .fontWeight(.bold)

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

    func ingredientRow(_ ingredient: Ingredient) -> some View {
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

    func instructionsSection(_ instructions: String) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Instructions")
                .font(.title2)
                .fontWeight(.bold)

            Text(instructions)
                .font(.body)
        }
    }

    func notesSection(_ notes: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Notes")
                .font(.title3)
                .fontWeight(.bold)

            Text(notes)
                .font(.body)
                .foregroundColor(.secondary)
        }
    }
}
