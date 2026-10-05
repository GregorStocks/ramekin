import PhotosUI
import SwiftUI

struct RecipeFormView: View {
    var onSaved: (() -> Void)?

    @Environment(\.dismiss) private var dismiss
    @StateObject private var viewModel: RecipeFormViewModel
    @FocusState private var focusedSectionId: UUID?
    @State private var showingDiscardConfirmation = false

    init(mode: RecipeFormMode, onSaved: (() -> Void)? = nil) {
        self.onSaved = onSaved
        _viewModel = StateObject(wrappedValue: RecipeFormViewModel(mode: mode))
    }

    private static let errorSectionId = "recipe-form-error"

    var body: some View {
        // Save and Review live in the navigation bar, so a failure must be
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
                        dismiss()
                    }
                }
            }
            ToolbarItem(placement: .confirmationAction) {
                if viewModel.mode == .create && viewModel.draft == nil {
                    reviewButton
                } else {
                    Button(viewModel.isSaving ? "Saving…" : "Save") {
                        Task {
                            if await viewModel.save() {
                                onSaved?()
                                dismiss()
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
            Button("Discard Changes", role: .destructive) { dismiss() }
            Button("Keep Editing", role: .cancel) {}
        }
        .disabled(viewModel.isSaving)
        .overlay { if viewModel.isLoading { ProgressView("Loading recipe…") } }
        .task {
            await viewModel.start()
        }
        .onChange(of: viewModel.selectedPhotoItems) { items in
            if !items.isEmpty {
                Task { await viewModel.uploadSelectedPhotos() }
            }
        }
    }
    private var form: some View {
        Form {
            errorSection
                .id(Self.errorSectionId)
            if viewModel.mode == .create && viewModel.draft == nil {
                recipeTextSection
            } else {
                draftReviewSection
                titleSection
                descriptionSection
                metadataSection
                ratingSection
                if viewModel.draft != nil {
                    draftIngredientsSection
                } else {
                    ingredientsFormSection
                }
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
            Text("Type or paste a whole recipe: title, ingredients, instructions, and any details, then tap Review.")
        }
        .disabled(viewModel.isPreparing)
    }

    /// The step's primary action lives in the navigation bar so it stays
    /// reachable above the keyboard however long the pasted text is.
    @ViewBuilder private var reviewButton: some View {
        if viewModel.isPreparing {
            ProgressView().accessibilityLabel("Reading recipe")
        } else {
            Button("Review") {
                Task { await viewModel.prepareRecipe() }
            }
            .disabled(viewModel.recipeText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
    }

    @ViewBuilder private var draftReviewSection: some View {
        if let draft = viewModel.draft {
            Section("Review recipe") {
                Text("Check the details, then save. Weight estimates are available only for supported ingredients and quantities.")
                ForEach(Array(draft.warnings.enumerated()), id: \.offset) { _, warning in
                    Text(warning).foregroundStyle(.orange)
                }
                DisclosureGroup("Original recipe text") { Text(viewModel.recipeText) }
            }
        }
    }

    @ViewBuilder private var draftIngredientsSection: some View {
        if let draft = viewModel.draft {
            Section("Ingredients") {
                TextEditor(text: $viewModel.rawIngredients)
                    .frame(minHeight: 150).accessibilityLabel("Ingredients")
                if viewModel.rawIngredients == draft.rawIngredients {
                    ForEach(Array(draft.content.ingredients.enumerated()), id: \.offset) { index, ingredient in
                        Text(ingredient.formatted(
                            includeAlternatives: true,
                            includeNote: true,
                            derived: draft.derivedMeasurements.measurement(forIngredientAt: index)
                        ))
                        .font(.caption)
                    }
                } else {
                    Text("Weight estimates will be recalculated when you save.").font(.caption)
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
