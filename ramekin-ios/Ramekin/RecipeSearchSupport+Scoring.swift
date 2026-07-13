import Foundation

/// The Swift mirror of the canonical relevance scorer in
/// ramekin-core/src/search.rs, pinned by
/// shared-test-vectors/search-ranking.json. Scoring only orders rows that
/// membership already matched, so it uses plain substring containment on
/// normalized text (never LIKE patterns) and flattened human-facing
/// ingredient text (never the JSONB match text), exactly like the server.
extension RecipeSearchSupport {
    /// The searchable fields of one recipe as plain text — the Swift mirror
    /// of `SearchDoc` in ramekin-core/src/search.rs.
    struct ScoringDocument {
        let title: String
        let description: String?
        let tags: [String]
        /// One entry per ingredient: its full text (measurement
        /// amounts/units, item, note, section).
        let ingredients: [String]
        let instructions: String
        let notes: String?
    }

    private static let weightExactTitle: UInt32 = 100_000
    private static let weightTitlePhrase: UInt32 = 20_000
    private static let weightAllTokensInTitle: UInt32 = 10_000
    private static let weightTokenInTitle: UInt32 = 2_000
    private static let weightTokenInTag: UInt32 = 800
    private static let weightTokenInDescription: UInt32 = 400
    private static let weightTokenInIngredient: UInt32 = 200
    private static let weightTokenInInstructions: UInt32 = 50
    private static let weightTokenInNotes: UInt32 = 50

    private struct NormalizedScoringDocument {
        let title: [UInt32]
        let description: [UInt32]?
        let tags: [[UInt32]]
        let ingredients: [[UInt32]]
        let instructions: [UInt32]
        let notes: [UInt32]?
    }

    /// Mirror of `ramekin_core::search::relevance_score`.
    static func relevanceScore(textTokens: [String], document: ScoringDocument) -> UInt32 {
        if textTokens.isEmpty {
            return 0
        }

        let tokens = textTokens.map(normalizedScalars)
        let normalized = NormalizedScoringDocument(
            title: normalizedScalars(document.title),
            description: document.description.map(normalizedScalars),
            tags: document.tags.map(normalizedScalars),
            ingredients: document.ingredients.map(normalizedScalars),
            instructions: normalizedScalars(document.instructions),
            notes: document.notes.map(normalizedScalars)
        )

        var score: UInt32 = 0

        var phrase: [UInt32] = []
        for (index, token) in tokens.enumerated() {
            if index > 0 {
                phrase.append(0x20)
            }
            phrase.append(contentsOf: token)
        }
        if normalized.title == phrase {
            score += weightExactTitle
        } else if scalarsContain(normalized.title, phrase) {
            score += weightTitlePhrase
        }

        if tokens.allSatisfy({ scalarsContain(normalized.title, $0) }) {
            score += weightAllTokensInTitle
        }

        for token in tokens {
            score += tokenScore(token, in: normalized)
        }

        return score
    }

    private static func tokenScore(_ token: [UInt32], in document: NormalizedScoringDocument) -> UInt32 {
        var score: UInt32 = 0
        if scalarsContain(document.title, token) {
            score += weightTokenInTitle
        }
        if document.tags.contains(where: { scalarsContain($0, token) }) {
            score += weightTokenInTag
        }
        if let description = document.description, scalarsContain(description, token) {
            score += weightTokenInDescription
        }
        if document.ingredients.contains(where: { scalarsContain($0, token) }) {
            score += weightTokenInIngredient
        }
        if scalarsContain(document.instructions, token) {
            score += weightTokenInInstructions
        }
        if let notes = document.notes, scalarsContain(notes, token) {
            score += weightTokenInNotes
        }
        return score
    }

    /// Flatten one cached recipe into the scorer's field texts, mirroring the
    /// per-ingredient flattening in the server's relevance path.
    static func scoringDocument(for document: CachedRecipeSearchDocument) -> ScoringDocument {
        let ingredientTexts = document.ingredients.map { ingredient -> String in
            var parts: [String] = []
            for measurement in ingredient.measurements {
                if let amount = measurement.amount {
                    parts.append(amount)
                }
                if let unit = measurement.unit {
                    parts.append(unit)
                }
            }
            parts.append(ingredient.item)
            if let note = ingredient.note {
                parts.append(note)
            }
            if let section = ingredient.section {
                parts.append(section)
            }
            return parts.joined(separator: " ")
        }
        return ScoringDocument(
            title: document.summary.title,
            description: document.summary.description,
            tags: document.summary.tags,
            ingredients: ingredientTexts,
            instructions: document.instructions,
            notes: document.notes
        )
    }

    private static func scalarsContain(_ haystack: [UInt32], _ needle: [UInt32]) -> Bool {
        if needle.isEmpty {
            return true
        }
        if needle.count > haystack.count {
            return false
        }
        for start in 0...(haystack.count - needle.count) {
            var offset = 0
            while offset < needle.count, haystack[start + offset] == needle[offset] {
                offset += 1
            }
            if offset == needle.count {
                return true
            }
        }
        return false
    }
}
