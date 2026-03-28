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
    @State var showingDeleteConfirmation = false
    @State var isDeleting = false
    @State var deleteError: String?
    @State var isRescraping = false
    @State var rescrapeError: String?
    @State var showingRescrapeConfirmation = false

    var isViewingHistoricalVersion: Bool {
        RecipeVersionSupport.isViewingHistoricalVersion(
            displayedVersionId: recipe?.versionId,
            currentVersionId: currentVersionId
        )
    }

    var actionsDisabledForHistoricalVersion: Bool {
        isViewingHistoricalVersion || isReverting || isRescraping
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

                        if let sourceUrl = recipe.sourceUrl, !sourceUrl.isEmpty {
                            Button {
                                showingRescrapeConfirmation = true
                            } label: {
                                Label("Rescrape from Source", systemImage: "arrow.triangle.2.circlepath")
                            }
                            .disabled(actionsDisabledForHistoricalVersion || isRescraping)
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
        .confirmationDialog("Rescrape from Source", isPresented: $showingRescrapeConfirmation) {
            Button("Rescrape") {
                Task { await rescrapeFromSource() }
            }
        } message: {
            Text(
                "This will re-import the recipe from its original URL. "
                    + "Your current version will be preserved in history."
            )
        }
        .alert("Rescrape Failed", isPresented: Binding(
            get: { rescrapeError != nil },
            set: { if !$0 { rescrapeError = nil } }
        )) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(rescrapeError ?? "")
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
