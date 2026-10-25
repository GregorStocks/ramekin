import Combine
import Foundation
import PhotosUI
import SwiftUI

struct RecipeFormViewAPIClient {
    var createRecipe: (_ request: CreateRecipeRequest) async throws -> Void
    var updateRecipe: (_ id: UUID, _ request: UpdateRecipeRequest) async throws -> Void
    var getRecipe: (_ id: UUID) async throws -> RecipeResponse
    var syncTags: (_ cursor: Int64?) async throws -> SyncTagsResponse
    var uploadPhoto: (_ fileURL: URL) async throws -> UploadPhotoResponse
    var prepareTextRecipe: (_ text: String) async throws -> PrepareTextRecipeResponse = {
        try await ImportAPI.prepareTextRecipe(prepareTextRecipeRequest: PrepareTextRecipeRequest(text: $0))
    }

    static let live = RecipeFormViewAPIClient(
        createRecipe: {
            _ = try await RecipesAPI.createRecipe(createRecipeRequest: $0)
        },
        updateRecipe: {
            try await RecipesAPI.updateRecipe(id: $0, updateRecipeRequest: $1)
        },
        getRecipe: {
            try await RecipesAPI.getRecipe(id: $0)
        },
        syncTags: {
            try await SyncAPI.syncTags(cursor: $0)
        },
        uploadPhoto: {
            try await PhotosAPI.upload(file: $0)
        }
    )
}

@MainActor
final class RecipeFormViewModel: ObservableObject {
    let mode: RecipeFormMode

    @Published var formData: RecipeFormData
    @Published var availableTags: [SyncTag] = []
    @Published var selectedTagNamespace: String?
    @Published var newTagValue = ""
    @Published var newNamespace = ""
    @Published var isSaving = false
    @Published var isLoading = false
    @Published var error: String?
    @Published var selectedPhotoItems: [PhotosPickerItem] = []
    @Published var isUploadingPhoto = false
    @Published var recipeText = ""
    @Published var rawIngredients = ""
    @Published var draft: PrepareTextRecipeResponse?
    @Published var isPreparing = false
    /// A resumed edit started from an older version than the one now saved,
    /// so saving will replace someone else's changes.
    @Published var resumedOverNewerVersion = false

    /// The form as last loaded or prepared, to tell whether the user has
    /// edited anything since.
    private var savedFormData: RecipeFormData
    private let api: RecipeFormViewAPIClient
    /// Set while a resumed edit waits for the saved recipe to compare with.
    private var resumedEditVersionId: UUID?
    private var isResumingEdit = false

    init(mode: RecipeFormMode, resuming resumed: RecipeFormDraft? = nil, api: RecipeFormViewAPIClient = .live) {
        self.mode = mode
        self.api = api
        // One shared value: each RecipeFormData() starts with a fresh
        // ingredient row id, so two defaults would never compare equal.
        let initialFormData = RecipeFormData()
        _formData = Published(initialValue: resumed?.form ?? initialFormData)
        savedFormData = initialFormData
        if let resumed {
            recipeText = resumed.recipeText
            rawIngredients = resumed.rawIngredients
            draft = resumed.prepared
            if case .edit = mode {
                isResumingEdit = true
                resumedEditVersionId = resumed.form.expectedVersionId
            }
        }
    }
}

extension RecipeFormViewModel {
    /// Whether closing the form would throw away the user's work. In create
    /// mode that's any typed text or a prepared draft; a photo still
    /// uploading counts in either mode.
    var hasUnsavedChanges: Bool {
        if isUploadingPhoto { return true }
        return switch mode {
        case .create:
            draft != nil || !recipeText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case .edit:
            formData != savedFormData
        }
    }

    /// The form to keep if the app is terminated, or nil when there's nothing
    /// to lose. A photo mid-upload isn't kept; photos already added are.
    var resumableDraft: RecipeFormDraft? {
        guard hasUnsavedChanges else { return nil }
        return RecipeFormDraft(
            mode: mode,
            form: formData,
            recipeText: recipeText,
            rawIngredients: rawIngredients,
            prepared: draft
        )
    }

    var canSave: Bool {
        let hasRequiredVersion = switch mode {
        case .create:
            draft != nil && !rawIngredients.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case .edit:
            formData.expectedVersionId != nil
        }
        return !formData.title.trimmingCharacters(in: .whitespaces).isEmpty
            && !formData.instructions.trimmingCharacters(in: .whitespaces).isEmpty
            && hasRequiredVersion
            && !isSaving
            && !isPreparing
            // Saving mid-upload would drop the photo being uploaded.
            && !isUploadingPhoto
    }

    func prepareRecipe() async {
        error = nil
        isPreparing = true
        defer { isPreparing = false }
        do {
            let result = try await api.prepareTextRecipe(recipeText)
            formData = RecipeFormData(content: result.content)
            savedFormData = formData
            rawIngredients = result.rawIngredients
            draft = result
        } catch {
            self.error = APIErrorFormatter.userMessage(
                from: error, fallback: "Could not read the recipe. Your text is unchanged; try again."
            )
        }
    }

    func start() async {
        if let accountKey = AccountScope.currentAccountKey() {
            availableTags = TagFilterCache.loadAvailableTags(accountKey: accountKey)
        } else {
            availableTags = []
        }
        if case .edit(let recipeId) = mode {
            await loadRecipe(id: recipeId)
        }
        await loadAvailableTags()
    }

    /// Append a blank section heading with one empty ingredient under it and
    /// return the heading's id so the view can focus it.
    func addSection() -> UUID {
        let heading = IngredientEditorRow.newSection("")
        formData.ingredientRows.append(heading)
        formData.ingredientRows.append(.ingredient(.empty()))
        return heading.id
    }

    func addIngredient() {
        formData.ingredientRows.append(.ingredient(.empty()))
    }

    func addIngredient(toSectionWithId sectionId: UUID) {
        guard let sectionIndex = formData.ingredientRows.firstIndex(where: { $0.id == sectionId }) else {
            preconditionFailure("No section row with id \(sectionId)")
        }
        let end = ingredientSectionEndIndex(formData.ingredientRows, sectionIndex: sectionIndex)
        formData.ingredientRows.insert(.ingredient(.empty()), at: end)
    }

    /// Remove an ingredient, or a section heading. Removing a heading keeps its
    /// ingredients; they join the section above.
    func removeIngredientRow(id: UUID) {
        formData.ingredientRows.removeAll { $0.id == id }
    }

    func moveIngredientRows(from source: IndexSet, to destination: Int) {
        formData.ingredientRows.move(fromOffsets: source, toOffset: destination)
    }

    // Row bindings look rows up by id rather than index: SwiftUI can still read
    // a row's binding for a moment after the row is removed or moved. Such a read
    // gets the row's last rendered value, and a write to a removed row is dropped.

    func sectionNameBinding(id: UUID, rendered: String) -> Binding<String> {
        Binding(
            get: { [weak self] in
                guard let row = self?.ingredientRow(id: id), case .section(_, let name) = row else {
                    return rendered
                }
                return name
            },
            set: { [weak self] newValue in
                guard let self, let index = self.ingredientRowIndex(id: id) else { return }
                self.formData.ingredientRows[index] = .section(id: id, name: newValue)
            }
        )
    }

    func ingredientBinding(id: UUID, rendered: EditableIngredient) -> Binding<EditableIngredient> {
        Binding(
            get: { [weak self] in
                guard let row = self?.ingredientRow(id: id), case .ingredient(let ingredient) = row else {
                    return rendered
                }
                return ingredient
            },
            set: { [weak self] newValue in
                guard let self, let index = self.ingredientRowIndex(id: id) else { return }
                self.formData.ingredientRows[index] = .ingredient(newValue)
            }
        )
    }

    private func ingredientRowIndex(id: UUID) -> Int? {
        formData.ingredientRows.firstIndex { $0.id == id }
    }

    private func ingredientRow(id: UUID) -> IngredientEditorRow? {
        ingredientRowIndex(id: id).map { formData.ingredientRows[$0] }
    }

    func removePhoto(id photoId: UUID) {
        formData.photoIds.removeAll { $0 == photoId }
    }

    func addTag() {
        if selectedTagNamespace == nil {
            let parsed = TagHierarchySupport.parse(name: newTagValue)
            if parsed.namespace != nil {
                guard let normalizedTag = TagHierarchySupport.normalizedTypedName(from: newTagValue) else {
                    return
                }
                appendTagIfNeeded(normalizedTag)
                newTagValue = ""
                return
            }
        }

        guard let tag = TagHierarchySupport.formattedName(
            namespace: resolvedSelectedNamespace,
            value: newTagValue
        ) else {
            return
        }

        appendTagIfNeeded(tag)
        newTagValue = ""
        if selectedTagNamespace?.isEmpty == true {
            selectedTagNamespace = resolvedSelectedNamespace
            newNamespace = ""
        }
    }

    func loadAvailableTags() async {
        guard let accountKey = AccountScope.currentAccountKey() else {
            availableTags = []
            return
        }
        do {
            let tags = try await TagSyncSupport.refresh(accountKey: accountKey, fetch: api.syncTags)
            guard AccountScope.currentAccountKey() == accountKey else { return }
            availableTags = tags
        } catch is CancellationError {
            DebugLogger.shared.log("loadAvailableTags cancelled", source: "RecipeForm")
        } catch {
            DebugLogger.shared.log("loadAvailableTags error: \(error.localizedDescription)", source: "RecipeForm")
        }
    }

    func save() async -> Bool {
        error = nil
        isSaving = true
        do {
            switch mode {
            case .create:
                var request = formData.makeCreateRequest()
                request.rawIngredients = rawIngredients
                try await api.createRecipe(request)
            case .edit(let recipeId):
                try await api.updateRecipe(recipeId, try formData.makeUpdateRequest())
            }
            isSaving = false
            return true
        } catch is CancellationError {
            isSaving = false
            return false
        } catch {
            self.error = APIErrorFormatter.code(from: error) == .conflict
                ? "This recipe changed since you opened it. Your edits are still here; reload before saving again."
                : APIErrorFormatter.userMessage(from: error, fallback: "Failed to update recipe")
            isSaving = false
            return false
        }
    }

    func loadRecipe(id: UUID) async {
        isLoading = true
        do {
            let recipe = try await api.getRecipe(id)
            let loaded = RecipeFormData(recipe: recipe)
            if isResumingEdit {
                // Keep the resumed edits; the loaded recipe is what they'd replace.
                isResumingEdit = false
                resumedOverNewerVersion = recipe.versionId != resumedEditVersionId
                formData.expectedVersionId = recipe.versionId
            } else {
                formData = loaded
            }
            savedFormData = loaded
            isLoading = false
        } catch is CancellationError {
            isLoading = false
        } catch {
            self.error = error.localizedDescription
            isLoading = false
        }
    }

    func uploadSelectedPhotos() async {
        let items = selectedPhotoItems
        guard !items.isEmpty else {
            return
        }

        selectedPhotoItems = []
        await uploadPhotos(items)
    }

    func uploadPhotos(_ items: [PhotosPickerItem]) async {
        isUploadingPhoto = true
        for item in items {
            do {
                guard let data = try await item.loadTransferable(type: Data.self) else { continue }
                let fileURL = FileManager.default.temporaryDirectory
                    .appendingPathComponent(UUID().uuidString + ".jpg")
                try data.write(to: fileURL)
                let response = try await api.uploadPhoto(fileURL)
                formData.photoIds.append(response.id)
                try? FileManager.default.removeItem(at: fileURL)
            } catch is CancellationError {
                break
            } catch {
                self.error = "Photo upload failed: \(error.localizedDescription)"
            }
        }
        isUploadingPhoto = false
    }
}

private extension RecipeFormViewModel {
    var resolvedSelectedNamespace: String? {
        if let selectedTagNamespace {
            if selectedTagNamespace.isEmpty {
                return TagHierarchySupport.normalizedNamespace(from: newNamespace)
            }
            return selectedTagNamespace
        }
        return nil
    }

    func appendTagIfNeeded(_ name: String) {
        guard !formData.tags.contains(where: { $0.caseInsensitiveCompare(name) == .orderedSame }) else {
            return
        }
        formData.tags.append(name)
    }
}
