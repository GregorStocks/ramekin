import SwiftUI

extension RecipeDetailView {
    func recipeContent(_ recipe: RecipeResponse) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if !recipe.photoIds.isEmpty {
                PhotoCarouselView(photoIds: recipe.photoIds)
                    .frame(height: 250)
            }

            VStack(alignment: .leading, spacing: 20) {
                if isRescraping {
                    rescrapeProgressBanner()
                }

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

                if let nutritionalInfo = recipe.nutritionalInfoForDisplay {
                    Divider()
                    nutritionalInfoSection(nutritionalInfo)
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
                HStack(spacing: 6) {
                    Text("Servings: \(RecipeScaleSupport.scaleAmount(servings, by: recipeScale))")
                    if recipeScale != 1 {
                        scaleBadge
                    }
                }
                .font(.subheadline)
                .foregroundColor(.secondary)
            }

            if let rating = recipe.rating, (1...5).contains(rating) {
                HStack(spacing: 4) {
                    ForEach(1...5, id: \.self) { star in
                        Image(systemName: star <= rating ? "star.fill" : "star")
                            .font(.subheadline)
                            .foregroundColor(.orange)
                    }
                }
            }

            if let difficulty = recipe.difficulty, !difficulty.isEmpty {
                Text("Difficulty: \(difficulty)")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
        }
    }

    func tagsSection(_ tags: [String]) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(tags, id: \.self) { tag in
                    HierarchicalTagChip(name: tag)
                }
            }
        }
    }

    func ingredientsSection(_ ingredients: [Ingredient]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Text("Ingredients")
                    .font(.title2)
                    .fontWeight(.bold)
                if recipeScale != 1 {
                    scaleBadge
                }
            }

            scaleControls

            let grouped = groupIngredientsBySection(ingredients)

            ForEach(grouped, id: \.section) { group in
                if let section = group.section {
                    Text(section)
                        .font(.headline)
                        .padding(.top, 8)
                }

                ForEach(Array(group.items.enumerated()), id: \.offset) { _, ingredient in
                    ingredientRow(ingredient, scale: recipeScale)
                }
            }
        }
    }

    var scaleControls: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Text("Scale:")
                    .font(.subheadline)
                    .foregroundColor(.secondary)

                ForEach(RecipeScaleSupport.presets, id: \.value) { preset in
                    Button {
                        customScaleInput = ""
                        setRecipeScale(preset.value)
                    } label: {
                        Text(preset.label)
                            .font(.subheadline)
                            .fontWeight(recipeScale == preset.value ? .semibold : .regular)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .background(
                                Capsule()
                                    .fill(recipeScale == preset.value ? Color.orange : Color(.secondarySystemBackground))
                            )
                            .foregroundColor(recipeScale == preset.value ? .white : .primary)
                    }
                    .buttonStyle(.plain)
                }
            }

            HStack(spacing: 8) {
                TextField("Custom", text: $customScaleInput)
                    .keyboardType(.decimalPad)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 100)
                    .onSubmit(applyCustomScale)

                Button("Apply") {
                    applyCustomScale()
                }
                .buttonStyle(.bordered)
            }
        }
    }

    var scaleBadge: some View {
        Text("scaled \(RecipeScaleSupport.formatScaleLabel(recipeScale))")
            .font(.caption)
            .fontWeight(.semibold)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(Color.orange.opacity(0.16))
            .foregroundColor(.orange)
            .clipShape(Capsule())
    }

    func ingredientRow(_ ingredient: Ingredient, scale: Double) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Circle()
                .fill(Color.orange)
                .frame(width: 6, height: 6)
                .padding(.top, 6)

            VStack(alignment: .leading, spacing: 2) {
                Text(ingredient.formatted(scale: scale, includeAlternatives: true))
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

    func nutritionalInfoSection(_ nutritionalInfo: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Nutritional Info")
                .font(.title3)
                .fontWeight(.bold)

            Text(nutritionalInfo)
                .font(.body)
                .foregroundColor(.secondary)
        }
    }
}

extension RecipeResponse {
    var nutritionalInfoForDisplay: String? {
        guard let nutritionalInfo else { return nil }
        let trimmed = nutritionalInfo.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
