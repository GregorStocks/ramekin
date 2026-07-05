import SwiftUI

struct RecipeDetailView: View {
    let recipeId: UUID

    @Environment(\.dismiss) var dismiss
    @StateObject var viewModel: RecipeDetailViewModel

    init(recipeId: UUID) {
        self.recipeId = recipeId
        _viewModel = StateObject(wrappedValue: RecipeDetailViewModel(recipeId: recipeId))
    }

    var isViewingHistoricalVersion: Bool {
        viewModel.isViewingHistoricalVersion
    }

    var actionsDisabledForHistoricalVersion: Bool {
        viewModel.actionsDisabledForHistoricalVersion
    }

    var isAutoEnrichmentRunning: Bool {
        viewModel.isAutoEnrichmentRunning
    }

    var canCompareSelectedVersions: Bool {
        viewModel.canCompareSelectedVersions
    }

    var body: some View {
        ScrollView {
            if viewModel.isLoading && viewModel.recipe == nil {
                ProgressView()
                    .padding(.top, 100)
            } else if let error = viewModel.error, viewModel.recipe == nil {
                errorView(message: error)
            } else if let recipe = viewModel.recipe {
                recipeContent(recipe)
            }
        }
        .navigationTitle(viewModel.recipe?.title ?? "Recipe")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let recipe = viewModel.recipe {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Menu {
                        Button {
                            viewModel.showingEdit = true
                        } label: {
                            Label("Edit Recipe", systemImage: "pencil")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)
                        Button {
                            Task { await viewModel.enrichWithAI() }
                        } label: {
                            Label("Enrich with AI", systemImage: "wand.and.stars")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        Button {
                            viewModel.showingCustomEnrich = true
                        } label: {
                            Label("Customize with AI", systemImage: "wand.and.stars.inverse")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        Button {
                            Task { await viewModel.normalizeTitle() }
                        } label: {
                            Label("Auto-rename", systemImage: "textformat")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        Button {
                            Task { await viewModel.generateDescription() }
                        } label: {
                            Label("Generate Description", systemImage: "text.bubble")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        Button {
                            Task { await viewModel.generatePhoto() }
                        } label: {
                            Label("Generate AI Photo", systemImage: "photo.badge.plus")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        Button {
                            viewModel.showingAddToMealPlan = true
                        } label: {
                            Label("Add to Meal Plan", systemImage: "calendar.badge.plus")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        if !recipe.ingredients.isEmpty {
                            Button {
                                viewModel.showingAddToShoppingList = true
                            } label: {
                                Label("Add to Shopping List", systemImage: "cart.badge.plus")
                            }
                            .disabled(actionsDisabledForHistoricalVersion)
                        }

                        if let sourceUrl = recipe.sourceUrl, !sourceUrl.isEmpty {
                            Button {
                                viewModel.showingRescrapeConfirmation = true
                            } label: {
                                Label("Rescrape from Source", systemImage: "arrow.triangle.2.circlepath")
                            }
                            .disabled(actionsDisabledForHistoricalVersion || viewModel.isRescraping)
                        }

                        exportMenu(for: recipe)
                            .disabled(actionsDisabledForHistoricalVersion)
                        Divider()
                        Button(role: .destructive) {
                            viewModel.showingDeleteConfirmation = true
                        } label: {
                            Label("Delete Recipe", systemImage: "trash")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
        }
        .sheet(isPresented: $viewModel.showingAddToShoppingList) {
            if let recipe = viewModel.recipe {
                AddToShoppingListSheet(
                    recipe: recipe,
                    scale: viewModel.recipeScale,
                    isPresented: $viewModel.showingAddToShoppingList
                )
            }
        }
        .sheet(isPresented: $viewModel.showingAddToMealPlan) {
            if let recipe = viewModel.recipe {
                AddToMealPlanSheet(recipe: recipe, isPresented: $viewModel.showingAddToMealPlan)
            }
        }
        .sheet(isPresented: $viewModel.showingCustomEnrich) {
            if let recipe = viewModel.recipe {
                CustomEnrichSheet(recipe: recipe, isPresented: $viewModel.showingCustomEnrich) { result in
                    viewModel.enrichResult = result
                }
            }
        }
        .sheet(isPresented: $viewModel.showingEdit) {
            NavigationStack {
                RecipeFormView(mode: .edit(recipeId: recipeId)) {
                    Task { await viewModel.loadRecipe() }
                }
            }
        }
        .sheet(isPresented: Binding(
            get: { viewModel.enrichResult != nil },
            set: { if !$0 { viewModel.enrichResult = nil } }
        )) {
            if let recipe = viewModel.recipe, let modified = viewModel.enrichResult {
                EnrichPreviewSheet(
                    original: recipe,
                    modified: modified,
                    onApply: {
                        Task { await viewModel.applyEnrichment(modified) }
                    },
                    onCancel: { viewModel.enrichResult = nil }
                )
            }
        }
        .sheet(isPresented: $viewModel.showingCompareSheet) {
            RecipeVersionCompareSheet(
                olderVersion: viewModel.comparedOlderVersion,
                newerVersion: viewModel.comparedNewerVersion,
                isLoading: viewModel.isLoadingCompare,
                error: viewModel.compareError,
                onClose: viewModel.closeCompareSheet
            )
        }
        .confirmationDialog("Delete Recipe", isPresented: $viewModel.showingDeleteConfirmation) {
            Button("Delete Recipe", role: .destructive) {
                Task {
                    if await viewModel.deleteRecipe() {
                        dismiss()
                    }
                }
            }
        } message: {
            Text("Are you sure you want to delete this recipe? This cannot be undone.")
        }
        .alert("Delete Failed", isPresented: Binding(
            get: { viewModel.deleteError != nil },
            set: { if !$0 { viewModel.deleteError = nil } }
        )) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(viewModel.deleteError ?? "")
        }
        .confirmationDialog("Rescrape from Source", isPresented: $viewModel.showingRescrapeConfirmation) {
            Button("Rescrape") {
                viewModel.startRescrapeFromSource()
            }
        } message: {
            Text(
                "This will re-import the recipe from its original URL. "
                    + "Your current version will be preserved in history."
            )
        }
        .alert("Rescrape Failed", isPresented: Binding(
            get: { viewModel.rescrapeError != nil },
            set: { if !$0 { viewModel.rescrapeError = nil } }
        )) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(viewModel.rescrapeError ?? "")
        }
        .alert("AI Enrichment Failed", isPresented: Binding(
            get: { viewModel.autoEnrichError != nil },
            set: { if !$0 { viewModel.autoEnrichError = nil } }
        )) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(viewModel.autoEnrichError ?? "")
        }
        .alert(
            "Revert to this version?",
            isPresented: Binding(
                get: { viewModel.revertCandidate != nil },
                set: { if !$0 { viewModel.revertCandidate = nil } }
            ),
            presenting: viewModel.revertCandidate
        ) { version in
            Button("Cancel", role: .cancel) {
                viewModel.revertCandidate = nil
            }
            Button("Revert") {
                Task { await viewModel.revert(to: version) }
            }
        } message: { version in
            Text(
                "This will create a new current version with the content from "
                    + "\(formatDate(version.createdAt)). "
                    + "The current version will remain in history."
            )
        }
        .modifier(ExportPresentationModifier(
            shareItem: $viewModel.exportShareItem,
            errorMessage: $viewModel.exportError
        ))
        .task {
            await viewModel.loadRecipe()
        }
        .onChange(of: viewModel.isVersionHistoryExpanded) { isExpanded in
            if isExpanded && viewModel.versionHistory.isEmpty {
                Task { await viewModel.loadVersionHistory(force: true) }
            }
        }
        .onDisappear {
            viewModel.cancelRescrape()
        }
    }

    func setRecipeScale(_ value: Double) {
        viewModel.setRecipeScale(value)
    }

    func applyCustomScale() {
        viewModel.applyCustomScale()
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
