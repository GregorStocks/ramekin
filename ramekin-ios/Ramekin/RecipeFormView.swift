import PhotosUI
import SwiftUI

struct RecipeFormView: View {
    var onSaved: (() -> Void)?

    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.recipeFormDraftMemory) private var draftMemory
    @StateObject private var viewModel: RecipeFormViewModel
    @FocusState private var focusedSectionId: UUID?
    @State private var showingDiscardConfirmation = false
    /// Saved or discarded: a pending draft write must not bring it back.
    @State private var isClosing = false
    @State private var importTask: Task<Void, Never>?

    /// `resuming` reopens a form the app was terminated on.
    init(mode: RecipeFormMode, resuming draft: RecipeFormDraft? = nil, onSaved: (() -> Void)? = nil) {
        self.onSaved = onSaved
        _viewModel = StateObject(wrappedValue: RecipeFormViewModel(mode: mode, resuming: draft))
    }

    private static let errorSectionId = "recipe-form-error"

    var body: some View {
        // Save and Import live in the navigation bar, so a failure must be
        // brought into view rather than left at the bottom of a long form.
        ScrollViewReader { proxy in
            form
                .onChange(of: viewModel.error) { error in
                    guard error != nil else { return }
                    // Next run loop, once the error section exists to scroll to.
                    DispatchQueue.main.async {
                        withAnimation { proxy.scrollTo(Self.errorSectionId, anchor: .top) }
                    }
                }
        }
        .navigationTitle(viewModel.mode == .create ? "New Recipe" : "Edit Recipe")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") {
                    if viewModel.hasUnsavedChanges {
                        showingDiscardConfirmation = true
                    } else {
                        close()
                    }
                }
            }
            ToolbarItem(placement: .confirmationAction) {
                if viewModel.mode == .create {
                    importButton
                } else {
                    Button(viewModel.isSaving ? "Saving…" : "Save") {
                        Task {
                            if await viewModel.save() {
                                onSaved?()
                                close()
                            }
                        }
                    }
                    .disabled(!viewModel.canSave)
                    .fontWeight(.semibold)
                }
            }
        }
        .scrollDismissesKeyboard(.interactively)
        .interactiveDismissDisabled(viewModel.hasUnsavedChanges)
        .confirmationDialog(
            "Discard changes?",
            isPresented: $showingDiscardConfirmation,
            titleVisibility: .visible
        ) {
            Button("Discard Changes", role: .destructive) { close() }
            Button("Keep Editing", role: .cancel) {}
        }
        .disabled(viewModel.isSaving)
        .overlay { if viewModel.isLoading { ProgressView("Loading recipe…") } }
        .task {
            await viewModel.start()
            // Opening the form afresh replaces any draft it wasn't resumed from.
            rememberDraft()
        }
        .onChange(of: viewModel.selectedPhotoItems) { items in
            if !items.isEmpty {
                Task { await viewModel.uploadSelectedPhotos() }
            }
        }
        // The import job carries on without the form; the recipe appears
        // once it's saved.
        .onDisappear { importTask?.cancel() }
        // Writing scene storage re-renders every tab, so wait for a pause in
        // typing; leaving the foreground writes at once.
        .task(id: viewModel.resumableDraft) {
            try? await Task.sleep(nanoseconds: 500_000_000)
            guard !Task.isCancelled else { return }
            rememberDraft()
        }
        .onChange(of: scenePhase) { phase in
            if phase != .active { rememberDraft() }
        }
    }

    private func rememberDraft() {
        guard !isClosing else { return }
        draftMemory.remember(viewModel.mode, viewModel.resumableDraft)
    }

    private func close() {
        isClosing = true
        draftMemory.remember(viewModel.mode, nil)
        dismiss()
    }
    private var form: some View {
        Form {
            errorSection
                .id(Self.errorSectionId)
            if viewModel.mode == .create {
                recipeTextSection
                importProgressSection
            } else {
                titleSection
                descriptionSection
                metadataSection
                ratingSection
                ingredientsFormSection
                instructionsSection
                sourceSection
                tagsSection
                notesSection
                nutritionalInfoSection
                photosSection
            }
        }
    }
}

// MARK: - Form Sections

extension RecipeFormView {
    private var recipeTextSection: some View {
        Section {
            TextEditor(text: $viewModel.recipeText)
                .frame(minHeight: 240)
                .accessibilityLabel("Recipe text")
        } header: {
            Text("Recipe text")
        } footer: {
            Text("Type or paste a whole recipe: title, ingredients, instructions, and any details, then tap Import.")
        }
        .disabled(viewModel.isImporting)
    }

    /// The step's primary action lives in the navigation bar so it stays
    /// reachable above the keyboard however long the pasted text is.
    @ViewBuilder private var importButton: some View {
        if viewModel.isImporting {
            ProgressView().accessibilityLabel("Importing recipe")
        } else {
            Button("Import") {
                importTask = Task {
                    if await viewModel.importRecipe() {
                        onSaved?()
                        close()
                    }
                }
            }
            .disabled(!viewModel.canImport)
            .fontWeight(.semibold)
        }
    }

    @ViewBuilder private var importProgressSection: some View {
        if viewModel.isImporting {
            Section("Progress") {
                if viewModel.importSteps.isEmpty {
                    Text("Starting…")
                }
                ForEach(viewModel.importSteps, id: \.name) { step in
                    HStack {
                        switch step.status {
                        case "completed":
                            Image(systemName: "checkmark.circle.fill").foregroundColor(.green)
                        case "running":
                            ProgressView()
                        case "failed":
                            Image(systemName: "xmark.circle.fill").foregroundColor(.red)
                        default:
                            Image(systemName: "circle").foregroundColor(.secondary)
                        }
                        Text(NormalizeSelection.stepLabel(step.name, ops: []))
                    }
                }
            }
        }
    }

    private var titleSection: some View {
        Section { TextField("Recipe title", text: $viewModel.formData.title).font(.headline) }
            header: { Text("Title *") }
    }

    private var descriptionSection: some View {
        Section("Description") {
            TextField("Brief description", text: $viewModel.formData.recipeDescription, axis: .vertical)
                .lineLimit(2...4)
        }
    }

    private var metadataSection: some View {
        Section("Details") {
            TextField("Servings", text: $viewModel.formData.servings)
            TextField("Prep time", text: $viewModel.formData.prepTime).textContentType(.none)
            TextField("Cook time", text: $viewModel.formData.cookTime).textContentType(.none)
            TextField("Total time", text: $viewModel.formData.totalTime).textContentType(.none)
            TextField("Difficulty", text: $viewModel.formData.difficulty)
        }
    }

    private var ratingSection: some View {
        Section("Rating") {
            HStack(spacing: 0) {
                ForEach(1...5, id: \.self) { star in
                    Button {
                        viewModel.formData.rating = viewModel.formData.rating == star ? nil : star
                    } label: {
                        Image(systemName: star <= (viewModel.formData.rating ?? 0) ? "star.fill" : "star")
                            .font(.title2).foregroundColor(.accentColor)
                            .frame(minWidth: 44, minHeight: 44)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(star == 1 ? "1 star" : "\(star) stars")
                    .accessibilityAddTraits(viewModel.formData.rating == star ? .isSelected : [])
                }
                Spacer()
                if viewModel.formData.rating != nil {
                    Button("Clear") { viewModel.formData.rating = nil }
                        .buttonStyle(.borderless)
                        .font(.caption).foregroundColor(.secondary)
                }
            }
        }
    }

    /// Ingredients and section headings are rows of one list, so moving an
    /// ingredient past a heading (in edit mode) moves it into that section.
    private var ingredientsFormSection: some View {
        Section {
            let sectioned = sectionedIngredientIds(viewModel.formData.ingredientRows)
            ForEach(viewModel.formData.ingredientRows) { row in
                switch row {
                case .section(let id, let name):
                    sectionHeadingRow(
                        id: id,
                        name: viewModel.sectionNameBinding(id: id, rendered: name)
                    )
                case .ingredient(let ingredient):
                    IngredientRowView(
                        ingredient: viewModel.ingredientBinding(id: ingredient.id, rendered: ingredient)
                    ) {
                        viewModel.removeIngredientRow(id: ingredient.id)
                    }
                    .padding(.leading, sectioned.contains(ingredient.id) ? 12 : 0)
                }
            }
            .onMove { source, destination in
                viewModel.moveIngredientRows(from: source, to: destination)
            }
            addIngredientButton
            addSectionButton
        } header: {
            HStack {
                Text("Ingredients")
                Spacer()
                EditButton()
                    .frame(minHeight: 44)
            }
        }
    }

    private var instructionsSection: some View {
        Section { TextEditor(text: $viewModel.formData.instructions).frame(minHeight: 150)
            .accessibilityLabel("Instructions") }
            header: { Text("Instructions *") }
    }

    private var sourceSection: some View {
        Section("Source") {
            TextField("URL", text: $viewModel.formData.sourceUrl)
                .keyboardType(.URL).textContentType(.URL).autocapitalization(.none).autocorrectionDisabled()
            TextField("Source name", text: $viewModel.formData.sourceName)
        }
    }

    private var tagsSection: some View {
        RecipeFormTagsSection(
            tags: $viewModel.formData.tags,
            availableTags: $viewModel.availableTags,
            selectedTagNamespace: $viewModel.selectedTagNamespace,
            newTagValue: $viewModel.newTagValue,
            newNamespace: $viewModel.newNamespace,
            onAddTag: viewModel.addTag
        )
    }

    private var notesSection: some View {
        Section("Notes") { TextEditor(text: $viewModel.formData.notes).frame(minHeight: 80)
            .accessibilityLabel("Notes") }
    }

    private var nutritionalInfoSection: some View {
        Section("Nutritional Info") { TextEditor(text: $viewModel.formData.nutritionalInfo).frame(minHeight: 60)
            .accessibilityLabel("Nutritional Info") }
    }

    private var photosSection: some View {
        Section("Photos") {
            if !viewModel.formData.photoIds.isEmpty { photoGrid }
            PhotosPicker(selection: $viewModel.selectedPhotoItems, maxSelectionCount: 5, matching: .images) {
                Label(viewModel.isUploadingPhoto ? "Uploading…" : "Add Photo", systemImage: "photo.badge.plus")
            }
            .disabled(viewModel.isUploadingPhoto)
        }
    }

    @ViewBuilder
    private var errorSection: some View {
        if viewModel.resumedOverNewerVersion {
            Section {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundColor(.orange)
                    Text("This recipe changed since you started these edits. Saving will replace those changes.")
                }
            }
        }
        if let error = viewModel.error {
            Section {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundColor(.red)
                    Text(error).foregroundColor(.red)
                }
            }
        }
    }
}

// MARK: - Subview Helpers

extension RecipeFormView {
    private func sectionHeadingRow(id: UUID, name: Binding<String>) -> some View {
        HStack(spacing: 8) {
            TextField("Section name", text: name)
                .font(.headline)
                .foregroundColor(.accentColor)
                .focused($focusedSectionId, equals: id)
                .accessibilityLabel("Section name")
            Button {
                viewModel.addIngredient(toSectionWithId: id)
            } label: {
                Image(systemName: "plus.circle")
                    .frame(minWidth: 44, minHeight: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.borderless)
            .accessibilityLabel("Add ingredient to this section")
            Button {
                viewModel.removeIngredientRow(id: id)
            } label: {
                Image(systemName: "trash")
                    .foregroundColor(.red)
                    .frame(minWidth: 44, minHeight: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.borderless)
            .accessibilityLabel("Delete section")
        }
        // Headings are drop positions for ingredients but don't move themselves.
        .moveDisabled(true)
    }

    private var addIngredientButton: some View {
        Button {
            viewModel.addIngredient()
        } label: {
            Label("Add Ingredient", systemImage: "plus.circle")
        }
    }

    private var addSectionButton: some View {
        Button {
            focusedSectionId = viewModel.addSection()
        } label: {
            Label("Add Section", systemImage: "text.badge.plus")
        }
    }

    private var photoGrid: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 12) {
                let photoIds = viewModel.formData.photoIds
                ForEach(Array(photoIds.enumerated()), id: \.element) { index, photoId in
                    let position = "\(index + 1) of \(photoIds.count)"
                    ZStack(alignment: .topTrailing) {
                        RecipeThumbnail(photoId: photoId, size: 80)
                            .accessibilityElement(children: .ignore)
                            .accessibilityLabel("Photo \(position)")
                            .accessibilityAddTraits(.isImage)
                        Button { viewModel.removePhoto(id: photoId) } label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundColor(.white)
                                .background(Circle().fill(Color.black.opacity(0.6)))
                                .frame(width: 44, height: 44, alignment: .topTrailing)
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel("Remove photo \(position)")
                    }
                }
            }
        }
    }
}

#Preview("Create") {
    NavigationStack { RecipeFormView(mode: .create) }
}

#Preview("Edit") {
    NavigationStack { RecipeFormView(mode: .edit(recipeId: UUID())) }
}
