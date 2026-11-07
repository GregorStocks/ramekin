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
        revertRecipe: { try await RecipeVersionSupport.revertRecipe(id: $0, from: $1, expectedVersionId: $2) }
    )
}

/// The one sheet the recipe page shows at a time.
enum RecipeDetailSheet: String, Identifiable {
    case addToShoppingList
    case addToMealPlan
    case editWithAI
    case reNormalize
    case edit
    case compare

    var id: String { rawValue }
}

/// A recipe action under way. Only one runs at a time, and the others are
/// disabled until it finishes.
enum RecipeDetailAction: Equatable {
    case reverting
    case deleting
    case generatingPhoto
    case exporting
}

/// A failed action, reported in an alert. Load failures aren't alerts: they
/// show on the page, with Retry.
struct RecipeDetailAlert: Equatable {
    let title: String
    let message: String
}

@MainActor
final class RecipeDetailViewModel: ObservableObject {
    let recipeId: UUID

    @Published var recipe: RecipeResponse?
    @Published var currentVersionId: UUID?
    @Published var versionHistory: [VersionSummary] = []
    @Published var compareSelection: [UUID] = []
    @Published var isVersionHistoryExpanded = false
    /// True until a load, including its follow-up requests, has finished.
    @Published private(set) var isLoading = false
    /// Why the last load failed. Actions report failures in `actionAlert`.
    @Published private(set) var loadError: String?
    @Published var isLoadingVersions = false
    @Published var versionHistoryError: String?
    @Published private(set) var isLoadingCompare = false
    @Published private(set) var compareError: String?
    @Published private(set) var comparedOlderVersion: RecipeResponse?
    @Published private(set) var comparedNewerVersion: RecipeResponse?
    @Published var revertCandidate: VersionSummary?
    @Published var activeSheet: RecipeDetailSheet? {
        didSet {
            // However compare closes (Close or a swipe), drop its load.
            if oldValue == .compare && activeSheet != .compare {
                resetCompare()
            }
        }
    }
    @Published var showingDeleteConfirmation = false
    @Published private(set) var runningAction: RecipeDetailAction?
    @Published var actionAlert: RecipeDetailAlert?
    @Published var recipeScale: Double = 1
    @Published var customScaleInput = ""
    @Published var exportShareItem: ShareItem?
    @Published var exportError: String?

    private let api: RecipeDetailViewAPIClient
    /// Bumped per load, so only the latest load may write what it fetched.
    private var loadGeneration = 0
    /// Bumped per compare and when compare closes, likewise.
    private var compareGeneration = 0

    init(recipeId: UUID, api: RecipeDetailViewAPIClient = .live) {
        self.recipeId = recipeId
        self.api = api
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
        isViewingHistoricalVersion || runningAction != nil
    }

    var isReverting: Bool { runningAction == .reverting }
    var isDeleting: Bool { runningAction == .deleting }
    var isGeneratingPhoto: Bool { runningAction == .generatingPhoto }
    var isExporting: Bool { runningAction == .exporting }

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
        guard runningAction == nil else { return false }
        runningAction = .deleting
        defer { runningAction = nil }
        do {
            try await api.deleteRecipe(recipeId)
            NotificationCenter.default.post(name: .recipeDeleted, object: nil)
            return true
        } catch is CancellationError {
            return false
        } catch {
            actionAlert = RecipeDetailAlert(
                title: "Delete Failed",
                message: APIErrorFormatter.userMessage(from: error, fallback: "Failed to delete the recipe")
            )
            return false
        }
    }

    func loadRecipe(versionId: UUID? = nil) async {
        loadGeneration += 1
        let generation = loadGeneration
        isLoading = true
        loadError = nil
        defer {
            if generation == loadGeneration { isLoading = false }
        }

        do {
            let loaded = try await api.getRecipe(recipeId, versionId)
            guard generation == loadGeneration else { return }
            recipe = loaded

            if versionId == nil {
                currentVersionId = loaded.versionId
            } else if currentVersionId == nil {
                let current = try await api.getRecipe(recipeId, nil)
                guard generation == loadGeneration else { return }
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
        } catch {
            guard generation == loadGeneration else { return }
            loadError = APIErrorFormatter.userMessage(from: error, fallback: "Failed to load the recipe")
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

        resetCompare()
        let generation = compareGeneration
        activeSheet = .compare
        isLoadingCompare = true

        do {
            let first = try await api.getRecipe(recipeId, selectedVersionIds[0])
            guard generation == compareGeneration else { return }
            let second = try await api.getRecipe(recipeId, selectedVersionIds[1])
            guard generation == compareGeneration else { return }
            let orderedVersions = RecipeVersionSupport.sortForCompare(first, second)
            comparedOlderVersion = orderedVersions.older
            comparedNewerVersion = orderedVersions.newer
        } catch is CancellationError {
        } catch {
            guard generation == compareGeneration else { return }
            compareError = "Failed to load versions for comparison"
        }

        if generation == compareGeneration { isLoadingCompare = false }
    }

    func closeCompareSheet() {
        if activeSheet == .compare { activeSheet = nil }
    }

    private func resetCompare() {
        compareGeneration += 1
        isLoadingCompare = false
        compareError = nil
        comparedOlderVersion = nil
        comparedNewerVersion = nil
    }

    func revert(to version: VersionSummary) async {
        revertCandidate = nil
        guard runningAction == nil else { return }
        guard let currentVersionId else {
            actionAlert = RecipeDetailAlert(title: "Revert Failed", message: "Reload the recipe and try again.")
            return
        }
        runningAction = .reverting
        defer { runningAction = nil }

        do {
            let historicalRecipe = try await api.getRecipe(recipeId, version.id)
            try await api.revertRecipe(recipeId, historicalRecipe, currentVersionId)
        } catch is CancellationError {
            return
        } catch {
            actionAlert = RecipeDetailAlert(title: "Revert Failed", message: "Failed to revert to this version")
            return
        }
        await loadRecipe()
    }
}

extension RecipeDetailViewModel {
    func generatePhoto() async {
        guard recipe != nil, runningAction == nil else { return }
        runningAction = .generatingPhoto
        defer { runningAction = nil }

        do {
            _ = try await api.generatePhoto(recipeId)
        } catch is CancellationError {
            return
        } catch {
            actionAlert = RecipeDetailAlert(title: "AI Action Failed", message: "Failed to generate AI photo")
            return
        }
        await loadRecipe()
    }
}

extension RecipeDetailViewModel {
    func exportRecipeAsPaprika() async {
        guard runningAction == nil else { return }
        runningAction = .exporting
        defer { runningAction = nil }
        switch await ExportOutcome.download({ try await api.exportRecipe(recipeId) }) {
        case let .ready(item): exportShareItem = item
        case let .failed(message): exportError = message
        case .cancelled: break
        }
    }

    func exportRecipeAsPDF(_ recipe: RecipeResponse) async {
        guard runningAction == nil else { return }
        runningAction = .exporting
        defer { runningAction = nil }
        do {
            let cover = await api.fetchCoverPhoto(recipe)
            let result = try RecipePDFRenderer.render(recipe: recipe, coverPhoto: cover)
            exportShareItem = ShareItem(url: result.url)
        } catch {
            exportError = error.localizedDescription
        }
    }
}
