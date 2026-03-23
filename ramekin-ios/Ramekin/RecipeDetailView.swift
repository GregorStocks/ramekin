import SwiftUI

struct RecipeDetailView: View {
    let recipeId: UUID

    @State private var recipe: RecipeResponse?
    @State private var isLoading = false
    @State private var error: String?
    @State private var showingAddToShoppingList = false
    @State private var showingAddToMealPlan = false
    @State private var showingCustomEnrich = false
    @State private var showingEdit = false
    @State private var enrichResult: RecipeContent?
    @State private var isRescraping = false

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
            if let recipe = recipe {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Menu {
                        Button {
                            showingEdit = true
                        } label: {
                            Label("Edit Recipe", systemImage: "pencil")
                        }
                        Button {
                            showingCustomEnrich = true
                        } label: {
                            Label("Customize with AI", systemImage: "wand.and.stars")
                        }
                        Button {
                            showingAddToMealPlan = true
                        } label: {
                            Label("Add to Meal Plan", systemImage: "calendar.badge.plus")
                        }
                        if !recipe.ingredients.isEmpty {
                            Button {
                                showingAddToShoppingList = true
                            } label: {
                                Label("Add to Shopping List", systemImage: "cart.badge.plus")
                            }
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
        }
        .sheet(isPresented: $showingAddToShoppingList) {
            if let recipe = recipe {
                AddToShoppingListSheet(recipe: recipe, isPresented: $showingAddToShoppingList)
            }
        }
        .sheet(isPresented: $showingAddToMealPlan) {
            if let recipe = recipe {
                AddToMealPlanSheet(recipe: recipe, isPresented: $showingAddToMealPlan)
            }
        }
        .sheet(isPresented: $showingCustomEnrich) {
            if let recipe = recipe {
                CustomEnrichSheet(recipe: recipe, isPresented: $showingCustomEnrich) { result in
                    enrichResult = result
                }
            }
        }
        .sheet(isPresented: $showingEdit) {
            NavigationStack {
                RecipeFormView(mode: .edit(recipeId: recipeId)) {
                    Task { await loadRecipe() }
                }
            }
        }
        .sheet(isPresented: Binding(
            get: { enrichResult != nil },
            set: { if !$0 { enrichResult = nil } }
        )) {
            if let recipe = recipe, let modified = enrichResult {
                EnrichPreviewSheet(
                    original: recipe,
                    modified: modified,
                    onApply: {
                        Task { await applyEnrichment(modified) }
                    },
                    onCancel: { enrichResult = nil }
                )
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
                if isRescraping {
                    RecipeDetailStatusBanner(message: "Rescraping recipe...", style: .progress)
                        .accessibilityIdentifier("recipe-detail-rescrape-progress")
                }

                if let error = error {
                    RecipeDetailStatusBanner(message: error, style: .error)
                }

                // Header
                headerSection(recipe)

                if let sourceUrl = recipe.sourceUrl, let url = URL(string: sourceUrl) {
                    sourceLinkSection(url: url, name: recipe.sourceName)
                }

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
        VStack(alignment: .leading, spacing: 12) {
            Link(destination: url) {
                HStack {
                    Image(systemName: "link")
                    Text(name ?? url.host ?? "View Original")
                    Spacer()
                    Image(systemName: "arrow.up.right.square")
                }
                .foregroundColor(.orange)
            }

            Button {
                Task { await rescrapeRecipe() }
            } label: {
                Label(isRescraping ? "Rescraping..." : "Rescrape", systemImage: "arrow.clockwise")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(isRescraping)
        }
    }

}

// MARK: - Helpers & Data Loading

extension RecipeDetailView {
    func groupIngredientsBySection(_ ingredients: [Ingredient]) -> [(section: String?, items: [Ingredient])] {
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

    func formatIngredient(_ ingredient: Ingredient) -> String {
        var parts: [String] = []

        if let measurement = ingredient.measurements.first {
            if let amount = measurement.amount, !amount.isEmpty {
                parts.append(amount)
            }
            if let unit = measurement.unit, !unit.isEmpty {
                parts.append(unit)
            }
        }

        // Show alternative measurements (e.g. weight) in parentheses
        if ingredient.measurements.count > 1 {
            let altTexts = ingredient.measurements.dropFirst().compactMap { alt -> String? in
                let altParts = [alt.amount, alt.unit].compactMap { $0 }.filter { !$0.isEmpty }
                return altParts.isEmpty ? nil : altParts.joined(separator: " ")
            }
            if !altTexts.isEmpty {
                parts.append("(\(altTexts.joined(separator: ", ")))")
            }
        }

        parts.append(ingredient.item)

        return parts.joined(separator: " ")
    }

    func loadRecipe() async {
        isLoading = true
        error = nil

        do {
            let loaded = try await RecipesAPI.getRecipe(id: recipeId)
            await MainActor.run {
                recipe = loaded
                isLoading = false
            }
        } catch is CancellationError {
            // Task was cancelled, not a real error
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
                isLoading = false
            }
        }
    }

    func rescrapeRecipe() async {
        guard let recipe = recipe, recipe.sourceUrl?.isEmpty == false else {
            await MainActor.run {
                error = "Recipe has no source URL to rescrape from"
            }
            return
        }

        await MainActor.run {
            isRescraping = true
            error = nil
        }

        do {
            let response = try await RecipesAPI.rescrape(id: recipe.id)
            try await waitForRescrapeCompletion(jobId: response.jobId)
            await loadRecipe()
        } catch is CancellationError {
            // Task was cancelled, not a real error
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
            }
        }

        await MainActor.run {
            isRescraping = false
        }
    }

    func waitForRescrapeCompletion(jobId: UUID) async throws {
        let timeout = Date().addingTimeInterval(120)

        while Date() < timeout {
            let job = try await ScrapeAPI.getScrape(id: jobId)

            if job.status == "completed" {
                return
            }

            if job.status == "failed" {
                let message = job.error ?? "Unknown error"
                throw RamekinAPI.APIError.httpError(500, "Rescrape failed: \(message)")
            }

            try await Task.sleep(nanoseconds: 500_000_000)
        }

        throw RamekinAPI.APIError.httpError(408, "Rescrape timed out")
    }

    func applyEnrichment(_ modified: RecipeContent) async {
        let updateRequest = UpdateRecipeRequest(
            cookTime: modified.cookTime,
            description: modified.description,
            difficulty: modified.difficulty,
            ingredients: modified.ingredients,
            instructions: modified.instructions,
            notes: modified.notes,
            nutritionalInfo: modified.nutritionalInfo,
            prepTime: modified.prepTime,
            rating: modified.rating,
            servings: modified.servings,
            sourceName: modified.sourceName,
            sourceUrl: modified.sourceUrl,
            tags: modified.tags,
            title: modified.title,
            totalTime: modified.totalTime
        )
        do {
            try await RecipesAPI.updateRecipe(id: recipeId, updateRecipeRequest: updateRequest)
            await MainActor.run {
                enrichResult = nil
            }
            await loadRecipe()
        } catch is CancellationError {
            // Task was cancelled, not a real error
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
            }
        }
    }
}

#Preview {
    NavigationStack {
        RecipeDetailView(recipeId: UUID())
    }
}
