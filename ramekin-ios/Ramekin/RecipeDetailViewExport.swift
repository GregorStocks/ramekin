import SwiftUI
import UIKit

extension RecipeDetailView {
    @ViewBuilder
    func exportMenu(for recipe: RecipeResponse) -> some View {
        Menu {
            Button {
                Task { await exportRecipeAsPaprika() }
            } label: {
                Label("Export as Paprika", systemImage: "doc.zipper")
            }
            .disabled(isExporting)
            Button {
                Task { await exportRecipeAsPDF(recipe) }
            } label: {
                Label("Export as PDF", systemImage: "doc.richtext")
            }
            .disabled(isExporting)
        } label: {
            Label("Export", systemImage: "square.and.arrow.up")
        }
    }

    @MainActor
    func exportRecipeAsPaprika() async {
        guard !isExporting else { return }
        isExporting = true
        defer { isExporting = false }
        do {
            let download = try await RamekinAPI.shared.exportRecipe(id: recipeId)
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

    @MainActor
    func exportRecipeAsPDF(_ recipe: RecipeResponse) async {
        guard !isExporting else { return }
        isExporting = true
        defer { isExporting = false }
        do {
            let cover = await fetchCoverPhoto(for: recipe)
            let result = try RecipePDFRenderer.render(recipe: recipe, coverPhoto: cover)
            exportShareItem = ShareItem(url: result.url)
        } catch {
            exportError = error.localizedDescription
        }
    }

    /// Fetch the first photo, if any, using the same authenticated path the
    /// detail view uses. Failures degrade gracefully — the PDF still renders
    /// without a cover image.
    private func fetchCoverPhoto(for recipe: RecipeResponse) async -> UIImage? {
        guard let photoId = recipe.photoIds.first,
              let baseURL = RamekinAPI.shared.serverURL,
              let url = URL(string: "\(baseURL)/api/photos/\(photoId.uuidString)") else {
            return nil
        }
        guard let token = RamekinAPI.shared.authToken else { return nil }
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
    }
}
