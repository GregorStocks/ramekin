import PhotosUI
import SwiftUI

struct RecipeFormView: View {
    var onSaved: (() -> Void)?

    @Environment(\.dismiss) private var dismiss
    @StateObject private var viewModel: RecipeFormViewModel

    init(mode: RecipeFormMode, onSaved: (() -> Void)? = nil) {
        self.onSaved = onSaved
        _viewModel = StateObject(wrappedValue: RecipeFormViewModel(mode: mode))
    }

    var body: some View {
        Form {
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
            errorSection
        }
        .navigationTitle(viewModel.mode == .create ? "New Recipe" : "Edit Recipe")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .navigationBarLeading) {
                Button("Cancel") { dismiss() }
            }
            ToolbarItem(placement: .navigationBarTrailing) {
                if viewModel.mode != .create || viewModel.draft != nil {
                    Button(viewModel.isSaving ? "Saving..." : "Save") {
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
        .disabled(viewModel.isSaving)
        .overlay { if viewModel.isLoading { ProgressView("Loading recipe...") } }
        .task {
            await viewModel.start()
        }
        .onChange(of: viewModel.selectedPhotoItems) { items in
            if !items.isEmpty {
                Task { await viewModel.uploadSelectedPhotos() }
            }
        }
    }
}

// MARK: - Form Sections

extension RecipeFormView {
    private var recipeTextSection: some View {
        Section("Recipe text") {
            TextEditor(text: $viewModel.recipeText)
                .frame(minHeight: 240)
                .accessibilityLabel("Recipe text")
            Text("Type or paste a whole recipe: title, ingredients, instructions, and any details.")
                .font(.caption).foregroundStyle(.secondary)
            Button(viewModel.isPreparing ? "Reading recipe…" : "Review recipe") {
                Task { await viewModel.prepareRecipe() }
            }
            .disabled(viewModel.isPreparing || viewModel.recipeText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
        .disabled(viewModel.isPreparing)
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
                    ForEach(Array(draft.content.ingredients.enumerated()), id: \.offset) { _, ingredient in
                        Text(ingredient.formatted(includeAlternatives: true, includeNote: true))
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
            HStack(spacing: 8) {
                ForEach(1...5, id: \.self) { star in
                    Button {
                        viewModel.formData.rating = viewModel.formData.rating == star ? nil : star
                    } label: {
                        Image(systemName: star <= (viewModel.formData.rating ?? 0) ? "star.fill" : "star")
                            .font(.title2).foregroundColor(.accentColor)
                    }
                    .buttonStyle(.plain)
                }
                Spacer()
                if viewModel.formData.rating != nil {
                    Button("Clear") { viewModel.formData.rating = nil }
                        .font(.caption).foregroundColor(.secondary)
                }
            }
        }
    }

    private var ingredientsFormSection: some View {
        Section {
            let grouped = groupIngredientsBySection(viewModel.formData.ingredients)
            ForEach(Array(grouped.enumerated()), id: \.offset) { _, group in
                ingredientGroupView(group)
            }
            addIngredientButton
            addSectionRow
        } header: {
            HStack {
                Text("Ingredients")
                Spacer()
                EditButton().font(.caption)
            }
        }
    }

    private var instructionsSection: some View {
        Section { TextEditor(text: $viewModel.formData.instructions).frame(minHeight: 150) }
            header: { Text("Instructions *") }
    }

    private var sourceSection: some View {
        Section("Source") {
            TextField("URL", text: $viewModel.formData.sourceUrl)
                .keyboardType(.URL).autocapitalization(.none).autocorrectionDisabled()
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
        Section("Notes") { TextEditor(text: $viewModel.formData.notes).frame(minHeight: 80) }
    }

    private var nutritionalInfoSection: some View {
        Section("Nutritional Info") { TextEditor(text: $viewModel.formData.nutritionalInfo).frame(minHeight: 60) }
    }

    private var photosSection: some View {
        Section("Photos") {
            if !viewModel.formData.photoIds.isEmpty { photoGrid }
            PhotosPicker(selection: $viewModel.selectedPhotoItems, maxSelectionCount: 5, matching: .images) {
                Label(viewModel.isUploadingPhoto ? "Uploading..." : "Add Photo", systemImage: "photo.badge.plus")
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
    @ViewBuilder
    private func ingredientGroupView(_ group: IngredientGroup) -> some View {
        if !group.section.isEmpty {
            TextField("Section name", text: Binding(
                get: { group.section },
                set: { newName in
                    for idx in group.indices { viewModel.formData.ingredients[idx].section = newName }
                }
            ))
            .font(.headline).foregroundColor(.accentColor)
        }

        ForEach(group.indices, id: \.self) { idx in
            IngredientRowView(ingredient: $viewModel.formData.ingredients[idx]) {
                viewModel.removeIngredient(at: idx)
            }
        }
        .onMove { source, destination in
            viewModel.moveIngredients(in: group, from: source, to: destination)
        }
    }

    private var addIngredientButton: some View {
        Button {
            viewModel.addIngredient()
        } label: {
            Label("Add Ingredient", systemImage: "plus.circle")
        }
    }

    @ViewBuilder
    private var addSectionRow: some View {
        if !viewModel.formData.ingredients.isEmpty {
            HStack {
                TextField("New section name", text: $viewModel.newSectionName)
                Button("Add Section") {
                    viewModel.addSection()
                }
                .disabled(viewModel.newSectionName.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    private var photoGrid: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 12) {
                ForEach(viewModel.formData.photoIds, id: \.self) { photoId in
                    ZStack(alignment: .topTrailing) {
                        RecipeThumbnail(photoId: photoId, size: 80)
                        Button { viewModel.removePhoto(id: photoId) } label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundColor(.white)
                                .background(Circle().fill(Color.black.opacity(0.6)))
                        }
                        .offset(x: 4, y: -4)
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
