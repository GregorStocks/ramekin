import SwiftUI

struct RecipeDetailView: View {
    let recipeId: UUID

    @State private var recipe: RecipeResponse?
    @State private var currentVersionId: UUID?
    @State private var versionHistory: [VersionSummary] = []
    @State private var compareSelection: [UUID] = []
    @State private var isVersionHistoryExpanded = false
    @State private var isLoading = false
    @State private var isLoadingVersions = false
    @State private var isLoadingCompare = false
    @State private var isReverting = false
    @State private var error: String?
    @State private var versionHistoryError: String?
    @State private var compareError: String?
    @State private var revertCandidate: VersionSummary?
    @State private var showingAddToShoppingList = false
    @State private var showingAddToMealPlan = false
    @State private var showingCustomEnrich = false
    @State private var showingCompareSheet = false
    @State private var enrichResult: RecipeContent?
    @State private var comparedOlderVersion: RecipeResponse?
    @State private var comparedNewerVersion: RecipeResponse?

    private var isViewingHistoricalVersion: Bool {
        RecipeVersionSupport.isViewingHistoricalVersion(
            displayedVersionId: recipe?.versionId,
            currentVersionId: currentVersionId
        )
    }

    private var actionsDisabledForHistoricalVersion: Bool {
        isViewingHistoricalVersion || isReverting
    }

    private var canCompareSelectedVersions: Bool {
        compareSelection.count == 2
    }

    var body: some View {
        ScrollView {
            if isLoading && recipe == nil {
                ProgressView()
                    .padding(.top, 100)
            } else if let error, recipe == nil {
                errorView(message: error)
            } else if let recipe {
                recipeContent(recipe)
            }
        }
        .navigationTitle(recipe?.title ?? "Recipe")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let recipe {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Menu {
                        Button {
                            showingCustomEnrich = true
                        } label: {
                            Label("Customize with AI", systemImage: "wand.and.stars")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        Button {
                            showingAddToMealPlan = true
                        } label: {
                            Label("Add to Meal Plan", systemImage: "calendar.badge.plus")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        if !recipe.ingredients.isEmpty {
                            Button {
                                showingAddToShoppingList = true
                            } label: {
                                Label("Add to Shopping List", systemImage: "cart.badge.plus")
                            }
                            .disabled(actionsDisabledForHistoricalVersion)
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
        }
        .sheet(isPresented: $showingAddToShoppingList) {
            if let recipe {
                AddToShoppingListSheet(recipe: recipe, isPresented: $showingAddToShoppingList)
            }
        }
        .sheet(isPresented: $showingAddToMealPlan) {
            if let recipe {
                AddToMealPlanSheet(recipe: recipe, isPresented: $showingAddToMealPlan)
            }
        }
        .sheet(isPresented: $showingCustomEnrich) {
            if let recipe {
                CustomEnrichSheet(recipe: recipe, isPresented: $showingCustomEnrich) { result in
                    enrichResult = result
                }
            }
        }
        .sheet(isPresented: Binding(
            get: { enrichResult != nil },
            set: { if !$0 { enrichResult = nil } }
        )) {
            if let recipe, let modified = enrichResult {
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
        .sheet(isPresented: $showingCompareSheet) {
            RecipeVersionCompareSheet(
                olderVersion: comparedOlderVersion,
                newerVersion: comparedNewerVersion,
                isLoading: isLoadingCompare,
                error: compareError,
                onClose: closeCompareSheet
            )
        }
        .alert(
            "Revert to this version?",
            isPresented: Binding(
                get: { revertCandidate != nil },
                set: { if !$0 { revertCandidate = nil } }
            ),
            presenting: revertCandidate
        ) { version in
            Button("Cancel", role: .cancel) {
                revertCandidate = nil
            }
            Button("Revert") {
                Task { await revert(to: version) }
            }
        } message: { version in
            Text(
                "This will create a new current version with the content from "
                    + "\(formatDate(version.createdAt)). "
                    + "The current version will remain in history."
            )
        }
        .task {
            await loadRecipe()
        }
        .onChange(of: isVersionHistoryExpanded) { isExpanded in
            if isExpanded && versionHistory.isEmpty {
                Task { await loadVersionHistory(force: true) }
            }
        }
    }

    private func recipeContent(_ recipe: RecipeResponse) -> some View {
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

}

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
        return URL(string: "\(baseURL)/api/photos/\(photoId.uuidString)")
    }
}

#Preview {
    NavigationStack {
        RecipeDetailView(recipeId: UUID())
    }
}
