import PhotosUI
import SwiftUI

// MARK: - RecipeFormView

struct RecipeFormView: View {
    let mode: RecipeFormMode
    var onSaved: (() -> Void)?

    @Environment(\.dismiss) private var dismiss

    @State private var title = ""
    @State private var recipeDescription = ""
    @State private var instructions = ""
    @State private var servings = ""
    @State private var prepTime = ""
    @State private var cookTime = ""
    @State private var totalTime = ""
    @State private var difficulty = ""
    @State private var rating: Int?
    @State private var sourceUrl = ""
    @State private var sourceName = ""
    @State private var tags: [String] = []
    @State private var newTag = ""
    @State private var notes = ""
    @State private var nutritionalInfo = ""
    @State private var ingredients: [EditableIngredient] = [.empty()]
    @State private var photoIds: [UUID] = []
    @State private var isSaving = false
    @State private var isLoading = false
    @State private var error: String?
    @State private var selectedPhotoItems: [PhotosPickerItem] = []
    @State private var isUploadingPhoto = false
    @State private var newSectionName = ""

    private var canSave: Bool {
        !title.trimmingCharacters(in: .whitespaces).isEmpty
            && !instructions.trimmingCharacters(in: .whitespaces).isEmpty
            && !isSaving
    }

    var body: some View {
        Form {
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
            errorSection
        }
        .navigationTitle(mode == .create ? "New Recipe" : "Edit Recipe")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .navigationBarLeading) {
                Button("Cancel") { dismiss() }
            }
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(isSaving ? "Saving..." : "Save") {
                    Task { await save() }
                }
                .disabled(!canSave)
                .fontWeight(.semibold)
            }
        }
        .disabled(isSaving)
        .overlay { if isLoading { ProgressView("Loading recipe...") } }
        .task {
            if case .edit(let recipeId) = mode {
                await loadRecipe(id: recipeId)
            }
        }
        .onChange(of: selectedPhotoItems) { items in
            if !items.isEmpty {
                Task { await uploadPhotos(items) }
                selectedPhotoItems = []
            }
        }
    }
}

// MARK: - Form Sections

extension RecipeFormView {
    private var titleSection: some View {
        Section { TextField("Recipe title", text: $title).font(.headline) }
            header: { Text("Title *") }
    }

    private var descriptionSection: some View {
        Section("Description") {
            TextField("Brief description", text: $recipeDescription, axis: .vertical)
                .lineLimit(2...4)
        }
    }

    private var metadataSection: some View {
        Section("Details") {
            TextField("Servings", text: $servings)
            TextField("Prep time", text: $prepTime).textContentType(.none)
            TextField("Cook time", text: $cookTime).textContentType(.none)
            TextField("Total time", text: $totalTime).textContentType(.none)
            TextField("Difficulty", text: $difficulty)
        }
    }

    private var ratingSection: some View {
        Section("Rating") {
            HStack(spacing: 8) {
                ForEach(1...5, id: \.self) { star in
                    Button {
                        rating = rating == star ? nil : star
                    } label: {
                        Image(systemName: star <= (rating ?? 0) ? "star.fill" : "star")
                            .font(.title2).foregroundColor(.orange)
                    }
                    .buttonStyle(.plain)
                }
                Spacer()
                if rating != nil {
                    Button("Clear") { rating = nil }
                        .font(.caption).foregroundColor(.secondary)
                }
            }
        }
    }

    private var ingredientsFormSection: some View {
        Section {
            let grouped = groupIngredientsBySection(ingredients)
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
        Section { TextEditor(text: $instructions).frame(minHeight: 150) }
            header: { Text("Instructions *") }
    }

    private var sourceSection: some View {
        Section("Source") {
            TextField("URL", text: $sourceUrl)
                .keyboardType(.URL).autocapitalization(.none).autocorrectionDisabled()
            TextField("Source name", text: $sourceName)
        }
    }

    private var tagsSection: some View {
        Section("Tags") {
            if !tags.isEmpty { tagChips }
            HStack {
                TextField("Add tag", text: $newTag)
                    .autocapitalization(.none).autocorrectionDisabled()
                    .onSubmit { addTag() }
                Button("Add") { addTag() }
                    .disabled(newTag.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    private var notesSection: some View {
        Section("Notes") { TextEditor(text: $notes).frame(minHeight: 80) }
    }

    private var nutritionalInfoSection: some View {
        Section("Nutritional Info") { TextEditor(text: $nutritionalInfo).frame(minHeight: 60) }
    }

    private var photosSection: some View {
        Section("Photos") {
            if !photoIds.isEmpty { photoGrid }
            PhotosPicker(selection: $selectedPhotoItems, maxSelectionCount: 5, matching: .images) {
                Label(isUploadingPhoto ? "Uploading..." : "Add Photo", systemImage: "photo.badge.plus")
            }
            .disabled(isUploadingPhoto)
        }
    }

    @ViewBuilder
    private var errorSection: some View {
        if let error = error {
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
                    for idx in group.indices { ingredients[idx].section = newName }
                }
            ))
            .font(.headline).foregroundColor(.orange)
        }

        ForEach(group.indices, id: \.self) { idx in
            IngredientRowView(ingredient: $ingredients[idx]) {
                ingredients.remove(at: idx)
            }
        }
        .onMove { source, destination in
            moveIngredients(in: group, from: source, to: destination)
        }
    }

    private var addIngredientButton: some View {
        Button {
            ingredients.append(.empty(section: ingredients.last?.section ?? ""))
        } label: {
            Label("Add Ingredient", systemImage: "plus.circle")
        }
    }

    @ViewBuilder
    private var addSectionRow: some View {
        if !ingredients.isEmpty {
            HStack {
                TextField("New section name", text: $newSectionName)
                Button("Add Section") {
                    let name = newSectionName.trimmingCharacters(in: .whitespaces)
                    if !name.isEmpty {
                        ingredients.append(.empty(section: name))
                        newSectionName = ""
                    }
                }
                .disabled(newSectionName.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    private var tagChips: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(tags, id: \.self) { tag in
                    HStack(spacing: 4) {
                        Text(tag).font(.caption)
                        Button { tags.removeAll { $0 == tag } } label: {
                            Image(systemName: "xmark.circle.fill").font(.caption)
                        }
                    }
                    .padding(.horizontal, 10).padding(.vertical, 4)
                    .background(Color.orange.opacity(0.2))
                    .foregroundColor(.orange).clipShape(Capsule())
                }
            }
        }
    }

    private var photoGrid: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 12) {
                ForEach(photoIds, id: \.self) { photoId in
                    ZStack(alignment: .topTrailing) {
                        RecipeThumbnail(photoId: photoId, size: 80)
                        Button { photoIds.removeAll { $0 == photoId } } label: {
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

// MARK: - Actions

extension RecipeFormView {
    private var formData: RecipeFormData {
        RecipeFormData(
            title: title,
            recipeDescription: recipeDescription,
            instructions: instructions,
            servings: servings,
            prepTime: prepTime,
            cookTime: cookTime,
            totalTime: totalTime,
            difficulty: difficulty,
            rating: rating,
            sourceUrl: sourceUrl,
            sourceName: sourceName,
            tags: tags,
            notes: notes,
            nutritionalInfo: nutritionalInfo,
            ingredients: ingredients,
            photoIds: photoIds
        )
    }

    private func apply(formData: RecipeFormData) {
        title = formData.title
        recipeDescription = formData.recipeDescription
        instructions = formData.instructions
        servings = formData.servings
        prepTime = formData.prepTime
        cookTime = formData.cookTime
        totalTime = formData.totalTime
        difficulty = formData.difficulty
        rating = formData.rating
        sourceUrl = formData.sourceUrl
        sourceName = formData.sourceName
        tags = formData.tags
        notes = formData.notes
        nutritionalInfo = formData.nutritionalInfo
        photoIds = formData.photoIds
        ingredients = formData.ingredients
    }

    private func addTag() {
        let tag = newTag.trimmingCharacters(in: .whitespaces).lowercased()
        if !tag.isEmpty && !tags.contains(tag) { tags.append(tag) }
        newTag = ""
    }

    private func save() async {
        error = nil
        isSaving = true
        do {
            switch mode {
            case .create:
                try await saveNewRecipe()
            case .edit(let recipeId):
                try await updateExistingRecipe(recipeId)
            }
        } catch is CancellationError {
            // ignore
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
                isSaving = false
            }
        }
    }

    private func saveNewRecipe() async throws {
        let request = formData.makeCreateRequest()
        _ = try await RecipesAPI.createRecipe(createRecipeRequest: request)
        await MainActor.run {
            isSaving = false
            onSaved?()
            dismiss()
        }
    }

    private func updateExistingRecipe(_ recipeId: UUID) async throws {
        let request = formData.makeUpdateRequest()
        try await RecipesAPI.updateRecipe(id: recipeId, updateRecipeRequest: request)
        await MainActor.run {
            isSaving = false
            onSaved?()
            dismiss()
        }
    }

    private func loadRecipe(id: UUID) async {
        isLoading = true
        do {
            let recipe = try await RecipesAPI.getRecipe(id: id)
            await MainActor.run {
                populateForm(from: recipe)
                isLoading = false
            }
        } catch is CancellationError {
            // ignore
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
                isLoading = false
            }
        }
    }

    private func populateForm(from recipe: RecipeResponse) {
        apply(formData: RecipeFormData(recipe: recipe))
    }

    private func uploadPhotos(_ items: [PhotosPickerItem]) async {
        await MainActor.run { isUploadingPhoto = true }
        for item in items {
            do {
                guard let data = try await item.loadTransferable(type: Data.self) else { continue }
                let tempDir = FileManager.default.temporaryDirectory
                let fileURL = tempDir.appendingPathComponent(UUID().uuidString + ".jpg")
                try data.write(to: fileURL)
                let response = try await PhotosAPI.upload(file: fileURL)
                await MainActor.run { photoIds.append(response.id) }
                try? FileManager.default.removeItem(at: fileURL)
            } catch is CancellationError {
                break
            } catch {
                await MainActor.run {
                    self.error = "Photo upload failed: \(error.localizedDescription)"
                }
            }
        }
        await MainActor.run { isUploadingPhoto = false }
    }

    private func moveIngredients(
        in group: IngredientGroup, from source: IndexSet, to destination: Int
    ) {
        var groupItems = group.indices.map { ingredients[$0] }
        groupItems.move(fromOffsets: source, toOffset: destination)
        for (offset, globalIdx) in group.indices.enumerated() {
            ingredients[globalIdx] = groupItems[offset]
        }
    }
}

#Preview("Create") {
    NavigationStack { RecipeFormView(mode: .create) }
}

#Preview("Edit") {
    NavigationStack { RecipeFormView(mode: .edit(recipeId: UUID())) }
}
