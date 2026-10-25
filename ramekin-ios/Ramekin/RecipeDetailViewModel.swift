import Combine
import Foundation
import UIKit

struct RecipeDetailViewAPIClient {
    var deleteRecipe: (UUID) async throws -> Void
    var exportRecipe: (UUID) async throws -> RamekinAPI.ExportDownload
    var fetchCoverPhoto: (RecipeResponse) async -> UIImage?
    var generatePhoto: (UUID) async throws -> GeneratePhotoResponse
    var getRecipe: (UUID, UUID?) async throws -> RecipeResponse
    var listVersions: (UUID) async throws -> VersionListResponse
    var revertRecipe: (UUID, RecipeResponse, UUID) async throws -> Void
    var updateRecipe: (UUID, UpdateRecipeRequest) async throws -> Void

    static let live = RecipeDetailViewAPIClient(
        deleteRecipe: { try await RecipesAPI.deleteRecipe(id: $0) },
        exportRecipe: { try await RamekinAPI.shared.exportRecipe(id: $0) },
        fetchCoverPhoto: { recipe in
            guard let photoId = recipe.photoIds.first,
                  let baseURL = RamekinAPI.shared.serverURL,
                  let url = URL(string: "\(baseURL)/api/photos/\(photoId.uuidString)"),
                  let token = RamekinAPI.shared.authToken else {
                return nil
            }

            var request = URLRequest(url: url)
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            RamekinAPI.shared.applyAccessHeaders(to: &request)

            do {
                let (data, response) = try await insecureSession.data(for: request)
                guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                    return nil
                }
                return UIImage(data: data)
            } catch {
                return nil
            }
        },
        generatePhoto: { try await RecipesAPI.generatePhoto(id: $0) },
        getRecipe: { try await RecipesAPI.getRecipe(id: $0, versionId: $1) },
        listVersions: { try await RecipesAPI.listVersions(id: $0) },
        revertRecipe: { try await RecipeVersionSupport.revertRecipe(id: $0, from: $1, expectedVersionId: $2) },
        updateRecipe: { try await RecipesAPI.updateRecipe(id: $0, updateRecipeRequest: $1) }
    )
}

@MainActor
final class RecipeDetailViewModel: ObservableObject {
    let recipeId: UUID

    @Published var recipe: RecipeResponse?
    @Published var currentVersionId: UUID?
    @Published var versionHistory: [VersionSummary] = []
    @Published var compareSelection: [UUID] = []
    @Published var isVersionHistoryExpanded = false
    @Published var isLoading = false
    @Published var isLoadingVersions = false
    @Published var isLoadingCompare = false
    @Published var isReverting = false
    @Published var error: String?
    @Published var versionHistoryError: String?
    @Published var compareError: String?
    @Published var revertCandidate: VersionSummary?
    @Published var showingAddToShoppingList = false
    @Published var showingAddToMealPlan = false
    @Published var showingEditWithAI = false
    @Published var showingReNormalize = false
    @Published var showingEdit = false
    @Published var showingCompareSheet = false
    @Published var enrichResult: RecipeContent?
    @Published var comparedOlderVersion: RecipeResponse?
    @Published var comparedNewerVersion: RecipeResponse?
    @Published var showingDeleteConfirmation = false
    @Published var isDeleting = false
    @Published var deleteError: String?
    @Published var isGeneratingPhoto = false
    @Published var aiActionError: String?
    @Published var recipeScale: Double = 1
    @Published var customScaleInput = ""
    @Published var isExporting = false
    @Published var exportShareItem: ShareItem?
    @Published var exportError: String?

    private let api: RecipeDetailViewAPIClient

    init(recipeId: UUID, api: RecipeDetailViewAPIClient = .live) {
        self.recipeId = recipeId
        self.api = api
    }
}

extension RecipeDetailViewModel {
    func submitUpdate(_ request: UpdateRecipeRequest) async throws {
        try await api.updateRecipe(recipeId, request)
    }

    var isViewingHistoricalVersion: Bool {
        RecipeVersionSupport.isViewingHistoricalVersion(
            displayedVersionId: recipe?.versionId,
            currentVersionId: currentVersionId
        )
    }

    var actionsDisabledForHistoricalVersion: Bool {
        isViewingHistoricalVersion || isReverting || isAiActionRunning || isDeleting
    }

    var isAiActionRunning: Bool {
        isGeneratingPhoto
    }

    var canCompareSelectedVersions: Bool {
        compareSelection.count == 2
    }

    var aiActionProgressLabel: String? {
        isGeneratingPhoto ? "Generating AI photo…" : nil
    }

    func toggleCompareSelection(versionId: UUID) {
        compareSelection = RecipeVersionSupport.toggleCompareSelection(
            compareSelection,
            versionId: versionId
        )
    }

    func setRecipeScale(_ value: Double) {
        guard RecipeScaleSupport.isValidScale(value) else {
            return
        }
        recipeScale = value
    }

    /// The custom scale field's text as a usable scale, or nil.
    var parsedCustomScale: Double? {
        guard let value = RecipeScaleSupport.parseDecimal(customScaleInput),
              RecipeScaleSupport.isValidScale(value) else {
            return nil
        }
        return value
    }

    func applyCustomScale() {
        guard let value = parsedCustomScale else { return }
        setRecipeScale(value)
    }
}

extension RecipeDetailViewModel {
    func deleteRecipe() async -> Bool {
        isDeleting = true
        do {
            try await api.deleteRecipe(recipeId)
            NotificationCenter.default.post(name: .recipeDeleted, object: nil)
            return true
        } catch {
            deleteError = error.localizedDescription
            isDeleting = false
            return false
        }
    }

    func loadRecipe(versionId: UUID? = nil) async {
        isLoading = true
        error = nil

        do {
            let loaded = try await api.getRecipe(recipeId, versionId)
            recipe = loaded
            isLoading = false

            if versionId == nil {
                currentVersionId = loaded.versionId
            } else if currentVersionId == nil {
                let current = try await api.getRecipe(recipeId, nil)
                currentVersionId = current.versionId
            }

            if RecipeVersionSupport.shouldRefreshVersionHistory(
                requestedVersionId: versionId,
                isVersionHistoryExpanded: isVersionHistoryExpanded,
                hasCachedVersionHistory: !versionHistory.isEmpty
            ) {
                await loadVersionHistory(force: true)
            }
        } catch is CancellationError {
            isLoading = false
        } catch {
            self.error = error.localizedDescription
            isLoading = false
        }
    }

    func loadVersionHistory(force: Bool = false) async {
        if isLoadingVersions {
            return
        }

        if !force && !versionHistory.isEmpty {
            return
        }

        isLoadingVersions = true
        versionHistoryError = nil

        do {
            let response = try await api.listVersions(recipeId)
            versionHistory = response.versions

            if let current = response.versions.first(where: { $0.isCurrent }) {
                currentVersionId = current.id
            }
        } catch is CancellationError {
        } catch {
            versionHistoryError = error.localizedDescription
        }

        isLoadingVersions = false
    }

    func displayVersion(_ version: VersionSummary) async {
        if version.isCurrent {
            await loadRecipe()
        } else {
            await loadRecipe(versionId: version.id)
        }
    }

    func openCompareSheet() async {
        guard canCompareSelectedVersions else {
            return
        }
        let selectedVersionIds = compareSelection

        showingCompareSheet = true
        isLoadingCompare = true
        compareError = nil
        comparedOlderVersion = nil
        comparedNewerVersion = nil

        do {
            let first = try await api.getRecipe(recipeId, selectedVersionIds[0])
            let second = try await api.getRecipe(recipeId, selectedVersionIds[1])
            let orderedVersions = RecipeVersionSupport.sortForCompare(first, second)
            comparedOlderVersion = orderedVersions.older
            comparedNewerVersion = orderedVersions.newer
        } catch is CancellationError {
        } catch {
            compareError = "Failed to load versions for comparison"
        }

        isLoadingCompare = false
    }

    func closeCompareSheet() {
        showingCompareSheet = false
        isLoadingCompare = false
        compareError = nil
        comparedOlderVersion = nil
        comparedNewerVersion = nil
    }

    func revert(to version: VersionSummary) async {
        revertCandidate = nil
        isReverting = true
        error = nil

        do {
            let historicalRecipe = try await api.getRecipe(recipeId, version.id)
            guard let currentVersionId else {
                error = "Reload the recipe and try again."
                isReverting = false
                return
            }
            try await api.revertRecipe(recipeId, historicalRecipe, currentVersionId)

            await loadRecipe()
        } catch is CancellationError {
        } catch {
            self.error = "Failed to revert to this version"
        }

        isReverting = false
    }
}

extension RecipeDetailViewModel {
    func generatePhoto() async {
        guard recipe != nil else { return }
        isGeneratingPhoto = true
        aiActionError = nil

        do {
            _ = try await api.generatePhoto(recipeId)
            await loadRecipe()
        } catch is CancellationError {
        } catch {
            aiActionError = "Failed to generate AI photo"
        }
        isGeneratingPhoto = false
    }
}

extension RecipeDetailViewModel {
    func exportRecipeAsPaprika() async {
        guard !isExporting else { return }
        isExporting = true
        defer { isExporting = false }
        do {
            let download = try await api.exportRecipe(recipeId)
            let url = try RecipeExportSupport.writeToTempFile(
                data: download.data,
                filename: download.filename
            )
            exportShareItem = ShareItem(url: url)
        } catch let apiError as RamekinAPI.APIError {
            exportError = apiError.errorDescription ?? "Export failed"
        } catch {
            exportError = error.localizedDescription
        }
    }

    func exportRecipeAsPDF(_ recipe: RecipeResponse) async {
        guard !isExporting else { return }
        isExporting = true
        defer { isExporting = false }
        do {
            let cover = await api.fetchCoverPhoto(recipe)
            let result = try RecipePDFRenderer.render(recipe: recipe, coverPhoto: cover)
            exportShareItem = ShareItem(url: result.url)
        } catch {
            exportError = error.localizedDescription
        }
    }
}
