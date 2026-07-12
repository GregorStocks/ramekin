import Foundation

/// Mirror of the server's parsed search query
/// (`ParsedQuery` in server/src/api/recipes/list.rs).
struct ParsedSearchQuery: Equatable {
    var textTokens: [String] = []
    var tags: [String] = []
    var source: String?
    var hasPhotos: Bool?
    /// Validated `yyyy-MM-dd` values naming inclusive UTC calendar days.
    var createdAfter: String?
    var createdBefore: String?
    var photoSize: NumericThreshold?
    var photoDim: NumericThreshold?

    /// `SyncRecipe` carries neither the source name nor detailed photo
    /// metadata, so queries using them can only run on the server.
    var requiresServer: Bool {
        source != nil || photoSize != nil || photoDim != nil
    }
}

/// The local mirror of server search: query parsing, result membership,
/// relevance scoring, and ordering over the cached recipe corpus. Every
/// behavior here is pinned to the server by shared vectors —
/// search-match-filter.json end to end, search-ranking.json for the scorer —
/// and normalization comes from the versioned contract in
/// SearchNormalizationSupport, so local results are the results the server
/// would have returned.
enum RecipeSearchSupport {
    // MARK: - Query parsing

    /// Split on spaces/tabs, respecting double quotes — mirrors the server's
    /// `tokenize`. The quote characters themselves are stripped.
    static func tokenize(_ input: String) -> [String] {
        var tokens: [String] = []
        var current = ""
        var inQuotes = false
        for character in input {
            if character == "\"" {
                inQuotes.toggle()
            } else if (character == " " || character == "\t") && !inQuotes {
                if !current.isEmpty {
                    tokens.append(current)
                    current = ""
                }
            } else {
                current.append(character)
            }
        }
        if !current.isEmpty {
            tokens.append(current)
        }
        return tokens
    }

    /// Mirror of the server's `parse_query`: classify each token as a DSL
    /// filter or a free-text term. Unknown `foo:` prefixes are text; filters
    /// with unparseable values are consumed as no-ops, exactly like the
    /// server.
    static func parse(_ query: String) -> ParsedSearchQuery {
        var result = ParsedSearchQuery()
        for token in tokenize(query) {
            if token.hasPrefix("tag:") {
                let tag = String(token.dropFirst("tag:".count))
                if !tag.isEmpty {
                    result.tags.append(tag)
                }
            } else if token.hasPrefix("source:") {
                let source = String(token.dropFirst("source:".count))
                if !source.isEmpty {
                    result.source = source
                }
            } else if token == "has:photos" || token == "has:photo" {
                result.hasPhotos = true
            } else if token == "no:photos" || token == "no:photo" {
                result.hasPhotos = false
            } else if token.hasPrefix("created:") {
                parseDateFilter(String(token.dropFirst("created:".count)), into: &result)
            } else if token.hasPrefix("photo_size:") {
                result.photoSize = parseNumericThreshold(String(token.dropFirst("photo_size:".count)))
            } else if token.hasPrefix("photo_dim:") {
                result.photoDim = parseNumericThreshold(String(token.dropFirst("photo_dim:".count)))
            } else if !token.isEmpty {
                result.textTokens.append(token)
            }
        }
        return result
    }

    private static func parseNumericThreshold(_ expression: String) -> NumericThreshold? {
        guard let first = expression.first,
              let comparison = NumericThresholdOperator(rawValue: String(first)),
              let value = Int32(expression.dropFirst())
        else {
            return nil
        }
        return NumericThreshold(comparison: comparison, value: Int(value))
    }

    private static func parseDateFilter(_ expression: String, into result: inout ParsedSearchQuery) {
        if let dotsRange = expression.range(of: "..") {
            if let start = canonicalDate(String(expression[..<dotsRange.lowerBound])) {
                result.createdAfter = start
            }
            if let end = canonicalDate(String(expression[dotsRange.upperBound...])) {
                result.createdBefore = end
            }
            return
        }
        if expression.hasPrefix(">") {
            if let date = canonicalDate(String(expression.dropFirst())) {
                result.createdAfter = date
            }
            return
        }
        if expression.hasPrefix("<") {
            if let date = canonicalDate(String(expression.dropFirst())) {
                result.createdBefore = date
            }
            return
        }
        if let date = canonicalDate(expression) {
            result.createdAfter = date
            result.createdBefore = date
        }
    }

    private static func canonicalDate(_ value: String) -> String? {
        guard let date = utcDateFormatter.date(from: value) else {
            return nil
        }
        return utcDateFormatter.string(from: date)
    }

    // MARK: - Result membership

    /// Whether a cached recipe belongs in the result set — the mirror of the
    /// server's SQL filters. Bare-text tokens AND together, each matching any
    /// of title, description, instructions, notes, or the ingredient match
    /// text (NULL fields never match, like SQL). Tags use whole-value CITEXT
    /// equality: case-insensitive but accent-sensitive, never unaccented.
    static func matches(_ document: CachedRecipeSearchDocument, parsed: ParsedSearchQuery) -> Bool {
        let summary = document.summary

        for tag in parsed.tags {
            let folded = SearchNormalizationSupport.citextFold(tag)
            guard summary.tags.contains(where: { SearchNormalizationSupport.citextFold($0) == folded }) else {
                return false
            }
        }

        if let hasPhotos = parsed.hasPhotos, hasPhotos != (summary.thumbnailPhotoId != nil) {
            return false
        }

        guard matchesCreatedDates(
            createdAt: summary.createdAt,
            createdAfter: parsed.createdAfter,
            createdBefore: parsed.createdBefore
        ) else {
            return false
        }

        if parsed.textTokens.isEmpty {
            return true
        }
        let fields: [[UInt32]?] = [
            normalizedScalars(summary.title),
            summary.description.map(normalizedScalars),
            normalizedScalars(document.instructions),
            document.notes.map(normalizedScalars),
            normalizedScalars(document.ingredientMatchText),
        ]
        return parsed.textTokens.allSatisfy { token in
            let pattern = membershipPattern(token)
            return fields.contains { field in
                guard let field else { return false }
                return likeContains(haystack: field, pattern: pattern)
            }
        }
    }

    /// Inclusive UTC-calendar-day window on the recipe's creation time —
    /// shared semantics with the server's created-date filters, pinned by
    /// shared-test-vectors/created-date-filter.json.
    static func matchesCreatedDates(createdAt: Date, createdAfter: String?, createdBefore: String?) -> Bool {
        if let after = createdAfter.flatMap(utcDateFormatter.date(from:)),
           utcCalendar.startOfDay(for: createdAt) < utcCalendar.startOfDay(for: after) {
            return false
        }
        if let before = createdBefore.flatMap(utcDateFormatter.date(from:)),
           utcCalendar.startOfDay(for: createdAt) > utcCalendar.startOfDay(for: before) {
            return false
        }
        return true
    }

    /// The server builds each token's SQL pattern by escaping LIKE
    /// metacharacters and *then* unaccenting the pattern, so an unaccent
    /// replacement that emits `%`, `_`, or `\` (e.g. fullwidth `％` → `%`)
    /// acts as a wildcard or escape. Mirror that order exactly.
    private static func membershipPattern(_ token: String) -> [UInt32] {
        let escaped = token
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "%", with: "\\%")
            .replacingOccurrences(of: "_", with: "\\_")
        return normalizedScalars(escaped)
    }

    private static let percent: UInt32 = 0x25
    private static let underscore: UInt32 = 0x5F
    private static let backslash: UInt32 = 0x5C

    /// `haystack ILIKE '%' || pattern || '%'` over already-normalized
    /// codepoints: `%` matches any run, `_` exactly one codepoint, `\` makes
    /// the next codepoint literal.
    static func likeContains(haystack: [UInt32], pattern: [UInt32]) -> Bool {
        var fullPattern: [UInt32] = [percent]
        fullPattern.append(contentsOf: pattern)
        fullPattern.append(percent)
        return likeMatches(haystack: haystack, pattern: fullPattern)
    }

    private static func likeMatches(haystack: [UInt32], pattern: [UInt32]) -> Bool {
        var textIndex = 0
        var patternIndex = 0
        var starPattern = -1
        var starText = -1

        while textIndex < haystack.count {
            if patternIndex < pattern.count, pattern[patternIndex] == percent {
                starPattern = patternIndex
                starText = textIndex
                patternIndex += 1
            } else if patternIndex < pattern.count, pattern[patternIndex] == underscore {
                patternIndex += 1
                textIndex += 1
            } else if patternIndex < pattern.count,
                      literal(in: pattern, at: patternIndex).scalar == haystack[textIndex] {
                patternIndex += literal(in: pattern, at: patternIndex).width
                textIndex += 1
            } else if starPattern >= 0 {
                starText += 1
                textIndex = starText
                patternIndex = starPattern + 1
            } else {
                return false
            }
        }

        while patternIndex < pattern.count, pattern[patternIndex] == percent {
            patternIndex += 1
        }
        return patternIndex == pattern.count
    }

    private static func literal(in pattern: [UInt32], at index: Int) -> (scalar: UInt32, width: Int) {
        if pattern[index] == backslash, index + 1 < pattern.count {
            return (pattern[index + 1], 2)
        }
        return (pattern[index], 1)
    }

    // MARK: - Relevance scoring

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

    /// Mirror of `ramekin_core::search::relevance_score`, pinned by
    /// shared-test-vectors/search-ranking.json.
    static func relevanceScore(textTokens: [String], document: ScoringDocument) -> UInt32 {
        if textTokens.isEmpty {
            return 0
        }

        let tokens = textTokens.map(normalizedScalars)
        let title = normalizedScalars(document.title)
        let description = document.description.map(normalizedScalars)
        let tags = document.tags.map(normalizedScalars)
        let ingredients = document.ingredients.map(normalizedScalars)
        let instructions = normalizedScalars(document.instructions)
        let notes = document.notes.map(normalizedScalars)

        var score: UInt32 = 0

        var phrase: [UInt32] = []
        for (index, token) in tokens.enumerated() {
            if index > 0 {
                phrase.append(0x20)
            }
            phrase.append(contentsOf: token)
        }
        if title == phrase {
            score += weightExactTitle
        } else if scalarsContain(title, phrase) {
            score += weightTitlePhrase
        }

        if tokens.allSatisfy({ scalarsContain(title, $0) }) {
            score += weightAllTokensInTitle
        }

        for token in tokens {
            if scalarsContain(title, token) {
                score += weightTokenInTitle
            }
            if tags.contains(where: { scalarsContain($0, token) }) {
                score += weightTokenInTag
            }
            if let description, scalarsContain(description, token) {
                score += weightTokenInDescription
            }
            if ingredients.contains(where: { scalarsContain($0, token) }) {
                score += weightTokenInIngredient
            }
            if scalarsContain(instructions, token) {
                score += weightTokenInInstructions
            }
            if let notes, scalarsContain(notes, token) {
                score += weightTokenInNotes
            }
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

    // MARK: - Execution

    /// Filter and order the cached corpus exactly like
    /// `list_recipes_blocking` on the server. A nil sortBy applies the server
    /// default: relevance when the query has text terms, updated-date
    /// descending otherwise. Relevance ignores sortDir; every ordering
    /// breaks ties by recipe id ascending (UUID byte order), and relevance
    /// breaks score ties by recency first.
    static func execute(
        documents: [CachedRecipeSearchDocument],
        parsed: ParsedSearchQuery,
        sortBy: SortBy?,
        sortDir: Direction?
    ) -> [RecipeSummary] {
        precondition(!parsed.requiresServer, "source/photo_size/photo_dim queries are server-only")

        let matched = documents.filter { matches($0, parsed: parsed) }
        let effectiveSortBy = sortBy ?? (parsed.textTokens.isEmpty ? .updatedAt : .relevance)
        let descending = (sortDir ?? .desc) == .desc

        switch effectiveSortBy {
        case .relevance:
            let scored = matched.map { document in
                (
                    score: relevanceScore(textTokens: parsed.textTokens, document: scoringDocument(for: document)),
                    summary: document.summary
                )
            }
            return scored.sorted { lhs, rhs in
                if lhs.score != rhs.score {
                    return lhs.score > rhs.score
                }
                if lhs.summary.updatedAt != rhs.summary.updatedAt {
                    return lhs.summary.updatedAt > rhs.summary.updatedAt
                }
                return lhs.summary.id.uuidString < rhs.summary.id.uuidString
            }.map(\.summary)
        case .title:
            return matched.map(\.summary).sorted { lhs, rhs in
                RecipeTitleSortSupport.areInIncreasingOrder(
                    lhsTitle: lhs.title,
                    lhsID: lhs.id,
                    rhsTitle: rhs.title,
                    rhsID: rhs.id,
                    descending: descending
                )
            }
        case .updatedAt:
            return matched.map(\.summary).sorted { lhs, rhs in
                compareDates(lhs.updatedAt, rhs.updatedAt, lhs.id, rhs.id, descending: descending)
            }
        case .createdAt:
            return matched.map(\.summary).sorted { lhs, rhs in
                compareDates(lhs.createdAt, rhs.createdAt, lhs.id, rhs.id, descending: descending)
            }
        case .rating:
            return matched.map(\.summary).sorted { lhs, rhs in
                compareRatingsNullsLast(lhs.rating, rhs.rating, lhs.id, rhs.id, descending: descending)
            }
        case .random:
            fatalError("random ordering is server-only; routing must not send it here")
        }
    }

    private static func compareDates(
        _ lhs: Date, _ rhs: Date, _ lhsID: UUID, _ rhsID: UUID, descending: Bool
    ) -> Bool {
        if lhs == rhs {
            return lhsID.uuidString < rhsID.uuidString
        }
        return descending ? lhs > rhs : lhs < rhs
    }

    /// Mirror of the server's `rating (a|de)sc NULLS LAST, id asc`.
    private static func compareRatingsNullsLast(
        _ lhs: Int?, _ rhs: Int?, _ lhsID: UUID, _ rhsID: UUID, descending: Bool
    ) -> Bool {
        switch (lhs, rhs) {
        case let (lhs?, rhs?) where lhs == rhs:
            return lhsID.uuidString < rhsID.uuidString
        case let (lhs?, rhs?):
            return descending ? lhs > rhs : lhs < rhs
        case (_?, nil):
            return true
        case (nil, _?):
            return false
        case (nil, nil):
            return lhsID.uuidString < rhsID.uuidString
        }
    }

    // MARK: - Shared helpers

    private static func normalizedScalars(_ text: String) -> [UInt32] {
        SearchNormalizationSupport.normalizeForSearch(text).unicodeScalars.map(\.value)
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

    private static let utcCalendar: Calendar = {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }()

    private static let utcDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)!
        return formatter
    }()
}
