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
        case "ai_photo":
            return "AI Photo"
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

    static func shouldRefreshVersionHistory(
        requestedVersionId: UUID?,
        isVersionHistoryExpanded: Bool,
        hasCachedVersionHistory: Bool
    ) -> Bool {
        requestedVersionId == nil && (isVersionHistoryExpanded || hasCachedVersionHistory)
    }

    @available(macOS 10.15, iOS 13.0, tvOS 13.0, watchOS 6.0, *)
    static func revertRecipe(
        id: UUID,
        from recipe: RecipeResponse,
        expectedVersionId: UUID
    ) async throws {
        try await RecipesAPI.revertRecipe(
            id: id,
            recipe: recipe,
            expectedVersionId: expectedVersionId
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

            lines.append(ingredient.formatted(includeAlternatives: true, includeNote: true))
        }

        return lines.joined(separator: "\n")
    }

    static func formatTags(_ tags: [String]) -> String {
        tags.joined(separator: ", ")
    }
}
