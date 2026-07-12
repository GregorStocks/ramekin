import Foundation

/// Mirror of the server's parsed search query
/// (`ParsedQuery` in server/src/api/recipes/list.rs).
struct ParsedSearchQuery: Equatable {
    var textTokens: [String] = []
    var tags: [String] = []
    var source: String?
    /// `.any` when the query has no photo-presence token, mirroring the
    /// server's absent filter.
    var photoFilter: PhotoFilter = .any
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
        for token in tokenize(query) where !applyFilterToken(token, to: &result) {
            if !token.isEmpty {
                result.textTokens.append(token)
            }
        }
        return result
    }

    /// Consumes a DSL filter token into `result`, returning false for plain
    /// text terms. Prefix order matches the server's `parse_query`.
    private static func applyFilterToken(_ token: String, to result: inout ParsedSearchQuery) -> Bool {
        if let tag = value(of: "tag:", in: token) {
            if !tag.isEmpty {
                result.tags.append(tag)
            }
            return true
        }
        if let source = value(of: "source:", in: token) {
            if !source.isEmpty {
                result.source = source
            }
            return true
        }
        if let photoFilter = photoPresenceFilter(token) {
            result.photoFilter = photoFilter
            return true
        }
        if let dateExpression = value(of: "created:", in: token) {
            parseDateFilter(dateExpression, into: &result)
            return true
        }
        if let sizeExpression = value(of: "photo_size:", in: token) {
            result.photoSize = parseNumericThreshold(sizeExpression)
            return true
        }
        if let dimExpression = value(of: "photo_dim:", in: token) {
            result.photoDim = parseNumericThreshold(dimExpression)
            return true
        }
        return false
    }

    private static func value(of prefix: String, in token: String) -> String? {
        token.hasPrefix(prefix) ? String(token.dropFirst(prefix.count)) : nil
    }

    private static func photoPresenceFilter(_ token: String) -> PhotoFilter? {
        switch token {
        case "has:photos", "has:photo":
            return .hasPhotos
        case "no:photos", "no:photo":
            return .noPhotos
        default:
            return nil
        }
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
    /// `membershipPatterns` are the tokens' prebuilt LIKE patterns — like the
    /// server, they are built once per query, not per row.
    static func matches(
        _ document: CachedRecipeSearchDocument,
        parsed: ParsedSearchQuery,
        membershipPatterns: [[UInt32]]
    ) -> Bool {
        let summary = document.summary

        for tag in parsed.tags {
            let folded = SearchNormalizationSupport.citextFold(tag)
            guard summary.tags.contains(where: { SearchNormalizationSupport.citextFold($0) == folded }) else {
                return false
            }
        }

        switch parsed.photoFilter {
        case .any:
            break
        case .hasPhotos where summary.thumbnailPhotoId == nil:
            return false
        case .noPhotos where summary.thumbnailPhotoId != nil:
            return false
        case .hasPhotos, .noPhotos:
            break
        }

        guard matchesCreatedDates(
            createdAt: summary.createdAt,
            createdAfter: parsed.createdAfter,
            createdBefore: parsed.createdBefore
        ) else {
            return false
        }

        if membershipPatterns.isEmpty {
            return true
        }
        let fields: [[UInt32]?] = [
            normalizedScalars(summary.title),
            summary.description.map(normalizedScalars),
            normalizedScalars(document.instructions),
            document.notes.map(normalizedScalars),
            normalizedScalars(document.ingredientMatchText)
        ]
        return membershipPatterns.allSatisfy { pattern in
            fields.contains { field in
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

        let membershipPatterns = parsed.textTokens.map(membershipPattern)
        let matched = documents.filter {
            matches($0, parsed: parsed, membershipPatterns: membershipPatterns)
        }
        let effectiveSortBy = sortBy ?? (parsed.textTokens.isEmpty ? .updatedAt : .relevance)
        let descending = (sortDir ?? .desc) == .desc

        switch effectiveSortBy {
        case .relevance:
            return rankedByRelevance(matched, textTokens: parsed.textTokens)
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

    private static func rankedByRelevance(
        _ matched: [CachedRecipeSearchDocument],
        textTokens: [String]
    ) -> [RecipeSummary] {
        let scored = matched.map { document in
            (
                score: relevanceScore(textTokens: textTokens, document: scoringDocument(for: document)),
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

    static func normalizedScalars(_ text: String) -> [UInt32] {
        SearchNormalizationSupport.normalizeForSearch(text).unicodeScalars.map(\.value)
    }

    static let utcCalendar: Calendar = {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }()

    static let utcDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)!
        return formatter
    }()
}
