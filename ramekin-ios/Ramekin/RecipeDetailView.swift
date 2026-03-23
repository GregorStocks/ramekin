import SwiftUI

struct RecipeDetailView: View {
    let recipeId: UUID

    @Environment(\.dismiss) var dismiss

    @State var recipe: RecipeResponse?
    @State var currentVersionId: UUID?
    @State var versionHistory: [VersionSummary] = []
    @State var compareSelection: [UUID] = []
    @State var isVersionHistoryExpanded = false
    @State var isLoading = false
    @State var isLoadingVersions = false
    @State var isLoadingCompare = false
    @State var isReverting = false
    @State var error: String?
    @State var versionHistoryError: String?
    @State var compareError: String?
    @State var revertCandidate: VersionSummary?
    @State var showingAddToShoppingList = false
    @State var showingAddToMealPlan = false
    @State var showingCustomEnrich = false
    @State var showingEdit = false
    @State var showingCompareSheet = false
    @State var enrichResult: RecipeContent?
    @State var comparedOlderVersion: RecipeResponse?
    @State var comparedNewerVersion: RecipeResponse?
    @State var isRescraping = false
    @State var rescrapeTask: Task<Void, Never>?
    @State var showingDeleteConfirmation = false
    @State var isDeleting = false
    @State var deleteError: String?

    var isViewingHistoricalVersion: Bool {
        RecipeVersionSupport.isViewingHistoricalVersion(
            displayedVersionId: recipe?.versionId,
            currentVersionId: currentVersionId
        )
    }

    var actionsDisabledForHistoricalVersion: Bool {
        isViewingHistoricalVersion || isReverting
    }

    var canCompareSelectedVersions: Bool {
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
                            showingEdit = true
                        } label: {
                            Label("Edit Recipe", systemImage: "pencil")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

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
                        Divider()
                        Button(role: .destructive) {
                            showingDeleteConfirmation = true
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
        .confirmationDialog("Delete Recipe", isPresented: $showingDeleteConfirmation) {
            Button("Delete Recipe", role: .destructive) {
                Task { await deleteRecipe() }
            }
        } message: {
            Text("Are you sure you want to delete this recipe? This cannot be undone.")
        }
        .alert("Delete Failed", isPresented: Binding(
            get: { deleteError != nil },
            set: { if !$0 { deleteError = nil } }
        )) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(deleteError ?? "")
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
        .onDisappear {
            rescrapeTask?.cancel()
            rescrapeTask = nil
        }
    }
}

extension RecipeDetailView {
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
}

#Preview {
    NavigationStack {
        RecipeDetailView(recipeId: UUID())
    }
}
