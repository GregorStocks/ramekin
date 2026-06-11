import Foundation

extension RamekinAPI {
    /// Result of a binary export download: the bytes plus a suggested filename
    /// that callers should use when writing to disk / presenting to the user.
    struct ExportDownload {
        let data: Data
        let filename: String
    }

    /// Download a single recipe in Paprika (.paprikarecipe) format.
    func exportRecipe(id: UUID) async throws -> ExportDownload {
        let (data, response) = try await performRequestWithResponse(
            method: "GET",
            path: "/api/recipes/\(id.uuidString)/export",
            acceptedStatusCodes: [200],
            logBody: false
        )
        let filename = RecipeExportSupport.suggestedFilename(
            from: response.value(forHTTPHeaderField: "Content-Disposition"),
            fallback: RecipeExportSupport.fallbackSingleRecipeFilename()
        )
        return ExportDownload(data: data, filename: filename)
    }

    /// Download all recipes in Paprika (.paprikarecipes) archive format.
    func exportAllRecipes() async throws -> ExportDownload {
        let (data, response) = try await performRequestWithResponse(
            method: "GET",
            path: "/api/recipes/export",
            acceptedStatusCodes: [200],
            logBody: false
        )
        let filename = RecipeExportSupport.suggestedFilename(
            from: response.value(forHTTPHeaderField: "Content-Disposition"),
            fallback: RecipeExportSupport.fallbackAllRecipesFilename()
        )
        return ExportDownload(data: data, filename: filename)
    }
}
