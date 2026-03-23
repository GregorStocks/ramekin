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
        var lines: [String] = []
        var currentSection: String?

        for ingredient in ingredients {
            if ingredient.section != currentSection {
                currentSection = ingredient.section

                if let currentSection, !currentSection.isEmpty {
                    lines.append("[\(currentSection)]")
                }
            }

            lines.append(formatIngredient(ingredient))
        }

        return lines.joined(separator: "\n")
    }

    static func formatTags(_ tags: [String]) -> String {
        tags.joined(separator: ", ")
    }

    private static func formatIngredient(_ ingredient: Ingredient) -> String {
        var parts: [String] = []

        if let measurement = ingredient.measurements.first {
            if let amount = measurement.amount, !amount.isEmpty {
                parts.append(amount)
            }
            if let unit = measurement.unit, !unit.isEmpty {
                parts.append(unit)
            }
        }

        if ingredient.measurements.count > 1 {
            let alternatives = ingredient.measurements.dropFirst().compactMap { measurement -> String? in
                let values = [measurement.amount, measurement.unit]
                    .compactMap { $0 }
                    .filter { !$0.isEmpty }

                guard !values.isEmpty else {
                    return nil
                }

                return values.joined(separator: " ")
            }

            if !alternatives.isEmpty {
                parts.append("(\(alternatives.joined(separator: ", ")))")
            }
        }

        parts.append(ingredient.item)

        if let note = ingredient.note, !note.isEmpty {
            parts.append("(\(note))")
        }

        return parts.joined(separator: " ")
    }
}
