import SwiftUI

struct RecipeDetailView: View {
    let recipeId: UUID

    @Environment(\.dismiss) var dismiss
    @Environment(\.recipeFormDraftMemory) var draftMemory
    @StateObject var viewModel: RecipeDetailViewModel
    @FocusState var customScaleFocused: Bool
    /// Unsaved edits from before the app was terminated, offered for resuming.
    @State var resumableEdit: RecipeFormDraft?
    @State var resumingEdit: RecipeFormDraft?

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

    var isAiActionRunning: Bool {
        viewModel.isAiActionRunning
    }

    var canCompareSelectedVersions: Bool {
        viewModel.canCompareSelectedVersions
    }

    /// Separate from body so body stays small enough to type-check.
    private var scrollContent: some View {
        ScrollViewReader { proxy in
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
            .coordinateSpace(name: RecipeScrollAnchor.coordinateSpace)
            .modifier(RecipeScrollRestorationModifier(recipeId: recipeId, proxy: proxy))
        }
    }

    var body: some View {
        scrollContent
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle(viewModel.recipe?.title ?? "Recipe")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let recipe = viewModel.recipe {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Edit") {
                        // A fresh edit replaces the one offered for resuming.
                        resumableEdit = nil
                        resumingEdit = nil
                        viewModel.showingEdit = true
                    }
                    .fontWeight(.semibold)
                    .disabled(actionsDisabledForHistoricalVersion)
                    .accessibilityLabel("Edit Recipe")
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Menu {
                        Button {
                            viewModel.showingReNormalize = true
                        } label: {
                            Label("Re-normalize…", systemImage: "arrow.triangle.2.circlepath")
                        }
                        .disabled(actionsDisabledForHistoricalVersion)

                        Button {
                            viewModel.showingEditWithAI = true
                        } label: {
                            Label("Edit with AI…", systemImage: "wand.and.stars.inverse")
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
                    .accessibilityLabel("More recipe actions")
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
        .sheet(isPresented: $viewModel.showingEditWithAI) {
            if let recipe = viewModel.recipe {
                ProposalJobSheet(
                    recipeId: recipeId,
                    mode: .edit(photoIds: recipe.photoIds),
                    isPresented: $viewModel.showingEditWithAI
                ) {
                    await viewModel.loadRecipe()
                }
            }
        }
        .sheet(isPresented: $viewModel.showingReNormalize) {
            ProposalJobSheet(recipeId: recipeId, mode: .normalize, isPresented: $viewModel.showingReNormalize) {
                await viewModel.loadRecipe()
            }
        }
        .sheet(isPresented: $viewModel.showingEdit) {
            NavigationStack {
                RecipeFormView(mode: .edit(recipeId: recipeId), resuming: resumingEdit) {
                    Task { await viewModel.loadRecipe() }
                }
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
        .alert("AI Action Failed", isPresented: Binding(
            get: { viewModel.aiActionError != nil },
            set: { if !$0 { viewModel.aiActionError = nil } }
        )) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(viewModel.aiActionError ?? "")
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
            resumableEdit = draftMemory.stored(.edit(recipeId: recipeId))
            await viewModel.loadRecipe()
        }
        .onChange(of: viewModel.isVersionHistoryExpanded) { isExpanded in
            if isExpanded && viewModel.versionHistory.isEmpty {
                Task { await viewModel.loadVersionHistory(force: true) }
            }
        }
        .modifier(RecipeScaleRestorationModifier(recipeId: recipeId, viewModel: viewModel))
    }

    func resumeEdit() {
        resumingEdit = resumableEdit
        resumableEdit = nil
        viewModel.showingEdit = true
    }

    func discardResumableEdit() {
        draftMemory.remember(.edit(recipeId: recipeId), nil)
        resumableEdit = nil
    }

    func setRecipeScale(_ value: Double) {
        viewModel.setRecipeScale(value)
    }

    func applyCustomScale() {
        customScaleFocused = false
        viewModel.applyCustomScale()
    }
}

/// Seeds the recipe's scale from the scene's restoration state and records
/// changes back. Kept as a ViewModifier so the detail view's body stays small
/// enough to type-check.
private struct RecipeScaleRestorationModifier: ViewModifier {
    let recipeId: UUID
    @ObservedObject var viewModel: RecipeDetailViewModel
    @Environment(\.recipePlaceMemory) private var placeMemory

    func body(content: Content) -> some View {
        content
            .onAppear {
                guard let stored = placeMemory.stored(recipeId) else { return }
                viewModel.customScaleInput = stored.customInput
                viewModel.setRecipeScale(stored.scale)
            }
            .onChange(of: viewModel.recipeScale) { _ in rememberScale() }
            .onChange(of: viewModel.customScaleInput) { _ in rememberScale() }
    }

    private func rememberScale() {
        placeMemory.update(recipeId) { place in
            place.scale = viewModel.recipeScale
            place.customInput = viewModel.customScaleInput
        }
    }
}

/// Ids for the points a recipe's scroll position is restored to.
enum RecipeScrollAnchor {
    static let coordinateSpace = "recipe-detail-scroll"

    static func ingredient(_ index: Int) -> String { "ingredient-\(index)" }
    static func step(_ index: Int) -> String { "step-\(index)" }
}

private struct RecipeScrollAnchorPositionsKey: PreferenceKey {
    static let defaultValue: [String: CGFloat] = [:]

    static func reduce(value: inout [String: CGFloat], nextValue: () -> [String: CGFloat]) {
        value.merge(nextValue()) { $1 }
    }
}

extension View {
    /// Makes this view a point the recipe's scroll position is remembered by.
    func recipeScrollAnchor(_ id: String) -> some View {
        self.id(id).background(GeometryReader { geometry in
            Color.clear.preference(
                key: RecipeScrollAnchorPositionsKey.self,
                value: [id: geometry.frame(in: .named(RecipeScrollAnchor.coordinateSpace)).minY]
            )
        })
    }
}

/// Scrolls back to the ingredient or step the user left the recipe at, once
/// it has rendered, and records the topmost one in view as they scroll. The
/// iOS 16 target has no scrollPosition(id:), hence anchors and scrollTo.
private struct RecipeScrollRestorationModifier: ViewModifier {
    let recipeId: UUID
    let proxy: ScrollViewProxy
    @Environment(\.recipePlaceMemory) private var placeMemory
    @Environment(\.scenePhase) private var scenePhase
    @State private var hasAppeared = false
    /// Stored anchor still to scroll to; nothing is recorded until then.
    @State private var pendingAnchor: String?
    @State private var topAnchor: String?

    func body(content: Content) -> some View {
        content
            .onAppear {
                guard !hasAppeared else { return }
                hasAppeared = true
                pendingAnchor = placeMemory.stored(recipeId)?.scrollAnchor
            }
            .onPreferenceChange(RecipeScrollAnchorPositionsKey.self) { positions in
                // Empty until the recipe has loaded and rendered.
                guard !positions.isEmpty else { return }
                if let anchor = pendingAnchor {
                    pendingAnchor = nil
                    // A recipe edited since may no longer have the anchor.
                    if positions[anchor] != nil {
                        topAnchor = anchor
                        // Next run loop, once this layout pass is done.
                        DispatchQueue.main.async { proxy.scrollTo(anchor, anchor: .top) }
                        return
                    }
                }
                topAnchor = SceneRestoration.topAnchor(positions)
            }
            // Writing scene storage re-renders every tab, so record only once
            // scrolling settles; leaving the foreground records at once.
            .task(id: topAnchor) {
                try? await Task.sleep(nanoseconds: 500_000_000)
                guard !Task.isCancelled else { return }
                rememberAnchor()
            }
            .onChange(of: scenePhase) { phase in
                if phase != .active { rememberAnchor() }
            }
    }

    private func rememberAnchor() {
        guard hasAppeared, pendingAnchor == nil,
              placeMemory.stored(recipeId)?.scrollAnchor != topAnchor else { return }
        placeMemory.update(recipeId) { $0.scrollAnchor = topAnchor }
    }
}

struct PhotoCarouselView: View {
    let photoIds: [UUID]

    var body: some View {
        TabView {
            ForEach(Array(photoIds.enumerated()), id: \.element) { index, photoId in
                AuthenticatedImage(url: photoURL(for: photoId))
                    .clipped()
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("Recipe photo \(index + 1) of \(photoIds.count)")
                    .accessibilityAddTraits(.isImage)
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
