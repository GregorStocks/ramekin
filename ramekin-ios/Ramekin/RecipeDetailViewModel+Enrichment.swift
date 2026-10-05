import Foundation

extension RecipeDetailViewModel {
    /// Saves the reviewed changes and returns nil, or returns why it failed so
    /// the review sheet, which covers the recipe, can show it.
    @discardableResult
    func applyEnrichment(_ modified: RecipeContent) async -> String? {
        guard let currentVersionId else {
            return "Reload the recipe and try again."
        }
        let updateRequest = UpdateRecipeRequest(
            cookTime: modified.cookTime,
            description: modified.description,
            difficulty: modified.difficulty,
            expectedVersionId: currentVersionId,
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
            try await submitUpdate(updateRequest)
            enrichResult = nil
            await loadRecipe()
            return nil
        } catch is CancellationError {
            return nil
        } catch {
            return APIErrorFormatter.userMessage(
                from: error,
                fallback: "Failed to apply enrichment"
            )
        }
    }
}
