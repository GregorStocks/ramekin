import Foundation

/// The Swift mirror of the canonical relevance scorer in
/// ramekin-core/src/search.rs, pinned by
/// shared-test-vectors/search-ranking.json. Scoring only orders rows that
/// membership already matched, so it uses plain substring containment on
/// normalized text (never LIKE patterns) and flattened human-facing
/// ingredient text (never the JSON match text), exactly like the server.
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

    /// Mirror of `ramekin_core::search::relevance_score`.
    static func relevanceScore(textTokens: [String], document: ScoringDocument) -> UInt32 {
        relevanceScore(
            tokens: textTokens.map(SearchNormalizationSupport.normalizedUTF8),
            text: NormalizedSearchText(document)
        )
    }

    /// The scorer over already-normalized tokens and fields.
    static func relevanceScore(tokens: [[UInt8]], text: NormalizedSearchText) -> UInt32 {
        if tokens.isEmpty {
            return 0
        }

        var score: UInt32 = 0

        var phrase: [UInt8] = []
        for (index, token) in tokens.enumerated() {
            if index > 0 {
                phrase.append(0x20)
            }
            phrase.append(contentsOf: token)
        }
        if text.title == phrase {
            score += weightExactTitle
        } else if bytesContain(text.title, phrase) {
            score += weightTitlePhrase
        }

        if tokens.allSatisfy({ bytesContain(text.title, $0) }) {
            score += weightAllTokensInTitle
        }

        for token in tokens {
            score += tokenScore(token, in: text)
        }

        return score
    }

    private static func tokenScore(_ token: [UInt8], in text: NormalizedSearchText) -> UInt32 {
        var score: UInt32 = 0
        if bytesContain(text.title, token) {
            score += weightTokenInTitle
        }
        if text.tags.contains(where: { bytesContain($0, token) }) {
            score += weightTokenInTag
        }
        if let description = text.description, bytesContain(description, token) {
            score += weightTokenInDescription
        }
        if text.ingredients.contains(where: { bytesContain($0, token) }) {
            score += weightTokenInIngredient
        }
        if bytesContain(text.instructions, token) {
            score += weightTokenInInstructions
        }
        if let notes = text.notes, bytesContain(notes, token) {
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

    /// Substring containment over normalized UTF-8, which is the same
    /// relation as containment over the strings' Unicode scalars.
    static func bytesContain(_ haystack: [UInt8], _ needle: [UInt8]) -> Bool {
        if needle.isEmpty {
            return true
        }
        if needle.count > haystack.count {
            return false
        }
        return haystack.withUnsafeBytes { haystackBytes in
            needle.withUnsafeBytes { needleBytes in
                memmem(haystackBytes.baseAddress, haystackBytes.count, needleBytes.baseAddress, needleBytes.count) != nil
            }
        }
    }
}
