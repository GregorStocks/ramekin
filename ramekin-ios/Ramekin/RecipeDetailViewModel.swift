import Combine
import Foundation
import UIKit

struct RecipeDetailViewAPIClient {
    var deleteRecipe: (UUID) async throws -> Void
    var enrichRecipe: (RecipeContent) async throws -> RecipeContent
    var exportRecipe: (UUID) async throws -> RamekinAPI.ExportDownload
    var fetchCoverPhoto: (RecipeResponse) async -> UIImage?
    var generateDescription: (UUID) async throws -> GenerateDescriptionResponse
    var generatePhoto: (UUID) async throws -> GeneratePhotoResponse
    var getRecipe: (UUID, UUID?) async throws -> RecipeResponse
    var getScrape: (UUID) async throws -> ScrapeJobResponse
    var listVersions: (UUID) async throws -> VersionListResponse
    var normalizeTitle: (UUID) async throws -> NormalizeTitleResponse
    var rescrape: (UUID) async throws -> RescrapeResponse
    var revertRecipe: (UUID, RecipeResponse) async throws -> Void
    var updateRecipe: (UUID, UpdateRecipeRequest) async throws -> Void

    static let live = RecipeDetailViewAPIClient(
        deleteRecipe: { try await RecipesAPI.deleteRecipe(id: $0) },
        enrichRecipe: { try await EnrichAPI.enrichRecipe(recipeContent: $0) },
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
        generateDescription: { try await RecipesAPI.generateDescription(id: $0) },
        generatePhoto: { try await RecipesAPI.generatePhoto(id: $0) },
        getRecipe: { try await RecipesAPI.getRecipe(id: $0, versionId: $1) },
        getScrape: { try await ScrapeAPI.getScrape(id: $0) },
        listVersions: { try await RecipesAPI.listVersions(id: $0) },
        normalizeTitle: { try await RecipesAPI.normalizeTitle(id: $0) },
        rescrape: { try await RecipesAPI.rescrape(id: $0) },
        revertRecipe: { try await RecipeVersionSupport.revertRecipe(id: $0, from: $1) },
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
    @Published var showingCustomEnrich = false
    @Published var showingEdit = false
    @Published var showingCompareSheet = false
    @Published var enrichResult: RecipeContent?
    @Published var comparedOlderVersion: RecipeResponse?
    @Published var comparedNewerVersion: RecipeResponse?
    @Published var showingDeleteConfirmation = false
    @Published var isDeleting = false
    @Published var deleteError: String?
    @Published var isRescraping = false
    @Published var rescrapeError: String?
    @Published var showingRescrapeConfirmation = false
    @Published var isEnriching = false
    @Published var isGeneratingPhoto = false
    @Published var isGeneratingDescription = false
    @Published var isNormalizingTitle = false
    @Published var autoEnrichError: String?
    @Published var recipeScale: Double = 1
    @Published var customScaleInput = ""
    @Published var isExporting = false
    @Published var exportShareItem: ShareItem?
    @Published var exportError: String?

    private let api: RecipeDetailViewAPIClient
    private var rescrapeTask: Task<Void, Never>?

    init(recipeId: UUID, api: RecipeDetailViewAPIClient = .live) {
        self.recipeId = recipeId
        self.api = api
    }

    deinit {
        rescrapeTask?.cancel()
    }
}

extension RecipeDetailViewModel {
    var isViewingHistoricalVersion: Bool {
        RecipeVersionSupport.isViewingHistoricalVersion(
            displayedVersionId: recipe?.versionId,
            currentVersionId: currentVersionId
        )
    }

    var actionsDisabledForHistoricalVersion: Bool {
        isViewingHistoricalVersion || isReverting || isRescraping || isAutoEnrichmentRunning
    }

    var isAutoEnrichmentRunning: Bool {
        isEnriching || isGeneratingPhoto || isGeneratingDescription || isNormalizingTitle
    }

    var canCompareSelectedVersions: Bool {
        compareSelection.count == 2
    }

    var autoEnrichmentProgressLabel: String? {
        if isEnriching { return "Enriching recipe..." }
        if isGeneratingPhoto { return "Generating AI photo..." }
        if isGeneratingDescription { return "Generating description..." }
        if isNormalizingTitle { return "Renaming recipe..." }
        return nil
    }

    func toggleCompareSelection(versionId: UUID) {
        compareSelection = RecipeVersionSupport.toggleCompareSelection(
            compareSelection,
            versionId: versionId
        )
    }

    func setRecipeScale(_ value: Double) {
        guard value.isFinite, value > 0 else {
            return
        }
        recipeScale = value
    }

    func applyCustomScale() {
        guard let value = RecipeScaleSupport.parseDecimal(customScaleInput),
              value.isFinite,
              value > 0 else {
            return
        }
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
            try await api.revertRecipe(recipeId, historicalRecipe)

            await loadRecipe()
        } catch is CancellationError {
        } catch {
            self.error = "Failed to revert to this version"
        }

        isReverting = false
    }

    func applyEnrichment(_ modified: RecipeContent) async {
        let updateRequest = UpdateRecipeRequest(
            cookTime: modified.cookTime,
            description: modified.description,
            difficulty: modified.difficulty,
            ingredients: modified.ingredients,
            instructions: modified.instructions,
            notes: modified.notes,
            nutritionalInfo: modified.nutritionalInfo,
            prepTime: modified.prepTime,
            rating: modified.rating,
            servings: modified.servings,
            sourceName: modified.sourceName,
            sourceUrl: modified.sourceUrl,
            tags: modified.tags,
            title: modified.title,
            totalTime: modified.totalTime
        )

        do {
            try await api.updateRecipe(recipeId, updateRequest)
            enrichResult = nil
            await loadRecipe()
        } catch is CancellationError {
        } catch {
            self.error = error.localizedDescription
        }
    }
}

extension RecipeDetailViewModel {
    func startRescrapeFromSource() {
        rescrapeTask?.cancel()
        rescrapeTask = Task { await rescrapeFromSource() }
    }

    func cancelRescrape() {
        rescrapeTask?.cancel()
        rescrapeTask = nil
    }

    func rescrapeFromSource() async {
        isRescraping = true
        rescrapeError = nil
        error = nil

        do {
            let response = try await api.rescrape(recipeId)
            let jobId = response.jobId

            let pollStartTime = Date()
            let timeoutInterval: TimeInterval = 120

            while true {
                try Task.checkCancellation()

                let job = try await api.getScrape(jobId)

                if job.status == "completed" {
                    await loadRecipe()
                    isRescraping = false
                    return
                } else if job.status == "failed" {
                    rescrapeError = job.error ?? "Unknown error"
                    isRescraping = false
                    return
                }

                if Date().timeIntervalSince(pollStartTime) > timeoutInterval {
                    rescrapeError = "Rescrape timed out"
                    isRescraping = false
                    return
                }

                try await Task.sleep(nanoseconds: 500_000_000)
            }
        } catch is CancellationError {
            isRescraping = false
        } catch {
            rescrapeError = "Failed to rescrape recipe"
            isRescraping = false
        }
    }

    func enrichWithAI() async {
        guard let recipe else { return }
        isEnriching = true
        autoEnrichError = nil

        let content = RecipeContent(
            cookTime: recipe.cookTime,
            description: recipe.description,
            difficulty: recipe.difficulty,
            ingredients: recipe.ingredients,
            instructions: recipe.instructions,
            notes: recipe.notes,
            nutritionalInfo: recipe.nutritionalInfo,
            prepTime: recipe.prepTime,
            rating: recipe.rating,
            servings: recipe.servings,
            sourceName: recipe.sourceName,
            sourceUrl: recipe.sourceUrl,
            tags: recipe.tags,
            title: recipe.title,
            totalTime: recipe.totalTime
        )

        do {
            let enriched = try await api.enrichRecipe(content)
            enrichResult = enriched
        } catch is CancellationError {
        } catch {
            autoEnrichError = "Failed to enrich recipe"
        }
        isEnriching = false
    }

    func generatePhoto() async {
        guard recipe != nil else { return }
        isGeneratingPhoto = true
        autoEnrichError = nil

        do {
            _ = try await api.generatePhoto(recipeId)
            await loadRecipe()
        } catch is CancellationError {
        } catch {
            autoEnrichError = "Failed to generate AI photo"
        }
        isGeneratingPhoto = false
    }

    func generateDescription() async {
        guard recipe != nil else { return }
        isGeneratingDescription = true
        autoEnrichError = nil

        do {
            let result = try await api.generateDescription(recipeId)
            if result.changed {
                await loadRecipe()
            }
        } catch is CancellationError {
        } catch {
            autoEnrichError = "Failed to generate description"
        }
        isGeneratingDescription = false
    }

    func normalizeTitle() async {
        guard recipe != nil else { return }
        isNormalizingTitle = true
        autoEnrichError = nil

        do {
            let result = try await api.normalizeTitle(recipeId)
            if result.changed {
                await loadRecipe()
            }
        } catch is CancellationError {
        } catch {
            autoEnrichError = "Failed to normalize title"
        }
        isNormalizingTitle = false
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
