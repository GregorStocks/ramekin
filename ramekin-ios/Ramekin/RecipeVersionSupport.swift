import Foundation

enum RecipeVersionSupport {
    static func sourceLabel(for source: String) -> String {
        switch source {
        case "user":
            return "User Edit"
        case "scrape":
            return "Imported"
        case "enrich", "enrichment":
            return "AI Enriched"
        default:
            return source
        }
    }

    static func isViewingHistoricalVersion(
        displayedVersionId: UUID?,
        currentVersionId: UUID?
    ) -> Bool {
        guard let displayedVersionId, let currentVersionId else {
            return false
        }

        return displayedVersionId != currentVersionId
    }

    static func toggleCompareSelection(
        _ selected: [UUID],
        versionId: UUID
    ) -> [UUID] {
        if selected.contains(versionId) {
            return selected.filter { $0 != versionId }
        }

        if selected.count >= 2 {
            return [selected[1], versionId]
        }

        return selected + [versionId]
    }

    static func sortForCompare(
        _ first: RecipeResponse,
        _ second: RecipeResponse
    ) -> (older: RecipeResponse, newer: RecipeResponse) {
        if first.updatedAt <= second.updatedAt {
            return (first, second)
        }

        return (second, first)
    }

    static func updateRequest(for recipe: RecipeResponse) -> UpdateRecipeRequest {
        UpdateRecipeRequest(
            cookTime: recipe.cookTime,
            description: recipe.description,
            difficulty: recipe.difficulty,
            ingredients: recipe.ingredients,
            instructions: recipe.instructions,
            notes: recipe.notes,
            nutritionalInfo: recipe.nutritionalInfo,
            photoIds: recipe.photoIds,
            prepTime: recipe.prepTime,
            rating: recipe.rating,
            servings: recipe.servings,
            sourceName: recipe.sourceName,
            sourceUrl: recipe.sourceUrl,
            tags: recipe.tags,
            title: recipe.title,
            totalTime: recipe.totalTime
        )
    }

    static func formatIngredients(_ ingredients: [Ingredient]) -> String {
        ingredients.map { ingredient in
            var parts: [String] = []

            if let measurement = ingredient.measurements.first {
                if let amount = measurement.amount, !amount.isEmpty {
                    parts.append(amount)
                }
                if let unit = measurement.unit, !unit.isEmpty {
                    parts.append(unit)
                }
            }

            parts.append(ingredient.item)

            return parts.joined(separator: " ")
        }
        .joined(separator: "\n")
    }

    static func formatTags(_ tags: [String]) -> String {
        tags.joined(separator: ", ")
    }
}
