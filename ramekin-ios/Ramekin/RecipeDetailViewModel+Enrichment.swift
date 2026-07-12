import Foundation

extension RecipeDetailViewModel {
    func applyEnrichment(_ modified: RecipeContent) async {
        guard let currentVersionId else {
            preconditionFailure("Cannot apply enrichment before loading the current recipe version")
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
        } catch is CancellationError {
        } catch {
            self.error = APIErrorFormatter.userMessage(
                from: error,
                fallback: "Failed to apply enrichment"
            )
        }
    }
}
