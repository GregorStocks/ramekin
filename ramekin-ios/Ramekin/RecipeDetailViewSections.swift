import SwiftUI

extension RecipeDetailView {
    func recipeContent(_ recipe: RecipeResponse) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if !recipe.photoIds.isEmpty {
                PhotoCarouselView(photoIds: recipe.photoIds)
                    .frame(height: 250)
            }

            VStack(alignment: .leading, spacing: 20) {
                statusBanners

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
                    ingredientsSection(recipe.ingredients, derived: recipe.derivedMeasurements)
                    Divider()
                }

                instructionsSection(recipe.instructions)

                // The sections after the steps are anchors too, so scrolling
                // to them isn't restored as the last step.
                if let notes = recipe.notes, !notes.isEmpty {
                    Divider()
                    notesSection(notes)
                        .recipeScrollAnchor("notes")
                }

                Divider()
                CalorieEstimateSection(request: EstimateCaloriesRequest(
                    ingredients: recipe.ingredients,
                    scale: viewModel.recipeScale,
                    servings: recipe.servings
                ))
                .recipeScrollAnchor("calories")

                if let nutritionalInfo = recipe.nutritionalInfoForDisplay {
                    Divider()
                    nutritionalInfoSection(nutritionalInfo)
                        .recipeScrollAnchor("nutrition")
                }

                if let sourceUrl = recipe.sourceUrl, let url = URL(string: sourceUrl) {
                    Divider()
                    sourceLinkSection(url: url, name: recipe.sourceName)
                        .recipeScrollAnchor("source")
                }
            }
            .padding()
        }
    }

    @ViewBuilder var statusBanners: some View {
        if resumableEdit != nil {
            resumeEditBanner
        }

        if viewModel.isRescraping {
            progressBanner("Rescraping from source…")
        }

        if viewModel.isFinishingRescrapeEnrichment {
            progressBanner("Adding an AI title, description, and tags…")
        }

        // Export and delete run after their menu or dialog closes, so
        // without this nothing would show that they're under way.
        if viewModel.isExporting {
            progressBanner("Preparing export…")
        }

        if viewModel.isDeleting {
            progressBanner("Deleting recipe…")
        }

        if let banner = viewModel.autoEnrichmentProgressLabel {
            autoEnrichmentProgressBanner(banner)
        }

        if let error = viewModel.error {
            inlineErrorBanner(message: error)
        }
    }

    func headerSection(_ recipe: RecipeResponse) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(recipe.title)
                .font(.title)
                .fontWeight(.bold)
                .accessibilityAddTraits(.isHeader)

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
                    Text("Servings: \(RecipeScaleSupport.scaleAmount(servings, by: viewModel.recipeScale))")
                    if viewModel.recipeScale != 1 {
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
                            .foregroundColor(.accentColor)
                    }
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("Rated \(rating) of 5")
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

    func ingredientsSection(_ ingredients: [Ingredient], derived: [DerivedMeasurement]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Text("Ingredients")
                    .font(.title2)
                    .fontWeight(.bold)
                    .accessibilityAddTraits(.isHeader)
                if viewModel.recipeScale != 1 {
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
                        .accessibilityAddTraits(.isHeader)
                }

                ForEach(group.items, id: \.offset) { index, ingredient in
                    ingredientRow(
                        ingredient,
                        scale: viewModel.recipeScale,
                        derived: derived.measurement(forIngredientAt: index)
                    )
                    .recipeScrollAnchor(RecipeScrollAnchor.ingredient(index))
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
                        viewModel.customScaleInput = ""
                        setRecipeScale(preset.value)
                    } label: {
                        Text(preset.label)
                            .font(.subheadline)
                            .fontWeight(viewModel.recipeScale == preset.value ? .semibold : .regular)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .background(
                                Capsule()
                                    .fill(
                                        viewModel.recipeScale == preset.value
                                            ? Color.accentColor
                                            : Color(.secondarySystemBackground)
                                    )
                            )
                            .foregroundColor(viewModel.recipeScale == preset.value ? Color(.systemBackground) : .primary)
                            .frame(minHeight: 44)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(viewModel.recipeScale == preset.value ? .isSelected : [])
                }
            }

            HStack(spacing: 8) {
                // A decimal pad has no Return key, so Apply is the way to
                // commit; it also dismisses the keyboard.
                TextField("Custom", text: $viewModel.customScaleInput)
                    .keyboardType(.decimalPad)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 100)
                    .fixedSize()
                    .focused($customScaleFocused)
                    .accessibilityLabel("Custom scale")

                Button("Apply") {
                    applyCustomScale()
                }
                .buttonStyle(.bordered)
                .disabled(viewModel.parsedCustomScale == nil)
            }

            if !viewModel.customScaleInput.isEmpty && viewModel.parsedCustomScale == nil {
                Text("Enter a number greater than 0 and at most \(RecipeScaleSupport.maximumScale.formatted()).")
                    .font(.caption)
                    .foregroundColor(.red)
            }
        }
    }

    var scaleBadge: some View {
        Text("scaled \(RecipeScaleSupport.formatScaleLabel(viewModel.recipeScale))")
            .font(.caption)
            .fontWeight(.semibold)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(Color.accentColor.opacity(0.16))
            .foregroundColor(.accentColor)
            .clipShape(Capsule())
    }

    func ingredientRow(_ ingredient: Ingredient, scale: Double, derived: Measurement?) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Circle()
                .fill(Color.accentColor)
                .frame(width: 6, height: 6)
                .padding(.top, 6)

            VStack(alignment: .leading, spacing: 2) {
                Text(ingredient.formatted(scale: scale, includeAlternatives: true, derived: derived))
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
                .accessibilityAddTraits(.isHeader)

            // One Text per line, which reads the same as one Text of them
            // all, so the scroll position can be restored to a step.
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(instructions.components(separatedBy: "\n").enumerated()), id: \.offset) { index, line in
                    Text(line.isEmpty ? " " : line)
                        .font(.body)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .accessibilityHidden(line.isEmpty)
                        .recipeScrollAnchor(RecipeScrollAnchor.step(index))
                }
            }
        }
    }

    var resumeEditBanner: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: "square.and.pencil")
                    .foregroundColor(.accentColor)
                    .accessibilityHidden(true)
                Text("You have unsaved edits to this recipe.")
                    .font(.subheadline)
            }
            HStack(spacing: 12) {
                Button("Resume Editing") { resumeEdit() }
                    .buttonStyle(.borderedProminent)
                Button("Discard", role: .destructive) { discardResumableEdit() }
                    .buttonStyle(.bordered)
            }
            .disabled(actionsDisabledForHistoricalVersion)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(Color.accentColor.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    func notesSection(_ notes: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Notes")
                .font(.title3)
                .fontWeight(.bold)
                .accessibilityAddTraits(.isHeader)

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
                .accessibilityAddTraits(.isHeader)

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
