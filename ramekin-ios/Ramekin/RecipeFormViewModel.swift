import Combine
import Foundation
import PhotosUI

struct RecipeFormViewAPIClient {
    var createRecipe: (_ request: CreateRecipeRequest) async throws -> Void
    var updateRecipe: (_ id: UUID, _ request: UpdateRecipeRequest) async throws -> Void
    var getRecipe: (_ id: UUID) async throws -> RecipeResponse
    var listAllTags: () async throws -> TagsListResponse
    var uploadPhoto: (_ fileURL: URL) async throws -> UploadPhotoResponse

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
        listAllTags: {
            try await TagsAPI.listAllTags()
        },
        uploadPhoto: {
            try await PhotosAPI.upload(file: $0)
        }
    )
}

@MainActor
final class RecipeFormViewModel: ObservableObject {
    let mode: RecipeFormMode

    @Published var formData = RecipeFormData()
    @Published var availableTags: [TagItem] = []
    @Published var selectedTagNamespace: String?
    @Published var newTagValue = ""
    @Published var newNamespace = ""
    @Published var isSaving = false
    @Published var isLoading = false
    @Published var error: String?
    @Published var selectedPhotoItems: [PhotosPickerItem] = []
    @Published var isUploadingPhoto = false
    @Published var newSectionName = ""

    private let api: RecipeFormViewAPIClient

    init(mode: RecipeFormMode, api: RecipeFormViewAPIClient = .live) {
        self.mode = mode
        self.api = api
    }
}

extension RecipeFormViewModel {
    var canSave: Bool {
        !formData.title.trimmingCharacters(in: .whitespaces).isEmpty
            && !formData.instructions.trimmingCharacters(in: .whitespaces).isEmpty
            && !isSaving
    }

    func start() async {
        availableTags = TagFilterCache.loadAvailableTags()
        if case .edit(let recipeId) = mode {
            await loadRecipe(id: recipeId)
        }
        await loadAvailableTags()
    }

    func addSection() {
        let name = newSectionName.trimmingCharacters(in: .whitespaces)
        if !name.isEmpty {
            formData.ingredients.append(.empty(section: name))
            newSectionName = ""
        }
    }

    func addIngredient() {
        formData.ingredients.append(.empty(section: formData.ingredients.last?.section ?? ""))
    }

    func removeIngredient(at index: Int) {
        formData.ingredients.remove(at: index)
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
        do {
            let response = try await api.listAllTags()
            availableTags = response.tags
            TagFilterCache.saveAvailableTags(response.tags)
        } catch is CancellationError {
        } catch {
        }
    }

    func save() async -> Bool {
        error = nil
        isSaving = true
        do {
            switch mode {
            case .create:
                try await api.createRecipe(formData.makeCreateRequest())
            case .edit(let recipeId):
                try await api.updateRecipe(recipeId, formData.makeUpdateRequest())
            }
            isSaving = false
            return true
        } catch is CancellationError {
            isSaving = false
            return false
        } catch {
            self.error = error.localizedDescription
            isSaving = false
            return false
        }
    }

    func loadRecipe(id: UUID) async {
        isLoading = true
        do {
            let recipe = try await api.getRecipe(id)
            formData = RecipeFormData(recipe: recipe)
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

    func moveIngredients(
        in group: IngredientGroup,
        from source: IndexSet,
        to destination: Int
    ) {
        var groupItems = group.indices.map { formData.ingredients[$0] }
        groupItems.move(fromOffsets: source, toOffset: destination)
        for (offset, globalIdx) in group.indices.enumerated() {
            formData.ingredients[globalIdx] = groupItems[offset]
        }
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
