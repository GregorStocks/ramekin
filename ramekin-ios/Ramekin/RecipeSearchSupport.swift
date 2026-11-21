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
    /// Inclusive per-serving calorie bounds.
    var caloriesMin: Double?
    var caloriesMax: Double?

    /// `SyncRecipe` carries neither the source name, detailed photo
    /// metadata, nor calorie estimates, so queries using them can only run
    /// on the server.
    var requiresServer: Bool {
        source != nil || photoSize != nil || photoDim != nil
            || caloriesMin != nil || caloriesMax != nil
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
        if let calorieExpression = value(of: "calories:", in: token) {
            parseCalorieFilter(calorieExpression, into: &result)
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

    /// Whether a cached recipe passes the query's structured filters — the
    /// mirror of the server's SQL filters other than bare text. Tags use
    /// whole-value case-folded equality (`citextFold`): case-insensitive but
    /// accent-sensitive, never unaccented.
    static func matchesFilters(_ entry: RecipeSearchCorpus.Entry, query: PreparedSearchQuery) -> Bool {
        let summary = entry.summary

        for tag in query.foldedTags where !entry.foldedTags.contains(tag) {
            return false
        }

        switch query.parsed.photoFilter {
        case .any:
            break
        case .hasPhotos where summary.thumbnailPhotoId == nil:
            return false
        case .noPhotos where summary.thumbnailPhotoId != nil:
            return false
        case .hasPhotos, .noPhotos:
            break
        }

        if query.createdAfterDay != nil || query.createdBeforeDay != nil {
            let createdDay = utcCalendar.startOfDay(for: summary.createdAt)
            if let after = query.createdAfterDay, createdDay < after {
                return false
            }
            if let before = query.createdBeforeDay, createdDay > before {
                return false
            }
        }
        return true
    }

    /// Bare-text membership: the tokens AND together, each matching any of
    /// title, description, instructions, notes, or the ingredient match text
    /// (NULL fields never match, like SQL).
    static func matchesText(_ text: RecipeSearchText, tokens: [[UInt8]]) -> Bool {
        let fields = text.fields
        return tokens.allSatisfy { token in
            bytesContain(fields.title, token)
                || fields.description.map { bytesContain($0, token) } ?? false
                || bytesContain(fields.instructions, token)
                || fields.notes.map { bytesContain($0, token) } ?? false
                || bytesContain(text.ingredientMatch, token)
        }
    }

    /// Start of the UTC calendar day a `yyyy-MM-dd` bound names. Created-date
    /// filters are an inclusive window of UTC days on the recipe's creation
    /// time — shared semantics with the server, pinned by
    /// shared-test-vectors/created-date-filter.json.
    static func utcDay(_ value: String?) -> Date? {
        value.flatMap(utcDateFormatter.date(from:)).map(utcCalendar.startOfDay(for:))
    }

    // MARK: - Execution

    /// Filter and order the cached corpus exactly like
    /// `list_recipes_blocking` on the server. A nil sortBy applies the server
    /// default: relevance when the query has text terms, updated-date
    /// descending otherwise. Relevance ignores sortDir; every ordering
    /// breaks ties by recipe id ascending (UUID byte order), and relevance
    /// breaks score ties by recency first.
    static func execute(
        corpus: RecipeSearchCorpus,
        parsed: ParsedSearchQuery,
        sortBy: SortBy?,
        sortDir: Direction?
    ) -> [RecipeSummary] {
        precondition(!parsed.requiresServer, "source/photo_size/photo_dim/calories queries are server-only")

        let query = PreparedSearchQuery(parsed)
        // Only a text query reads the normalized text, so browsing never
        // waits for the corpus to be normalized.
        let texts = query.tokens.isEmpty ? nil : corpus.searchTexts()
        var matched: [RecipeSearchCorpus.Entry] = []
        var matchedTexts: [RecipeSearchText] = []
        for (index, entry) in corpus.entries.enumerated() where matchesFilters(entry, query: query) {
            if let texts {
                guard matchesText(texts[index], tokens: query.tokens) else { continue }
                matchedTexts.append(texts[index])
            }
            matched.append(entry)
        }
        let effectiveSortBy = sortBy ?? (parsed.textTokens.isEmpty ? .updatedAt : .relevance)
        let descending = (sortDir ?? .desc) == .desc

        switch effectiveSortBy {
        case .relevance:
            return rankedByRelevance(matched, texts: matchedTexts, tokens: query.tokens)
        case .title:
            return matched
                .map { (key: RecipeTitleSortSupport.sortKey($0.summary.title), entry: $0) }
                .sorted { lhs, rhs in
                    RecipeTitleSortSupport.areInIncreasingOrder(
                        lhsKey: lhs.key,
                        lhsID: lhs.entry.idKey,
                        rhsKey: rhs.key,
                        rhsID: rhs.entry.idKey,
                        descending: descending
                    )
                }
                .map(\.entry.summary)
        case .updatedAt:
            return matched.sorted { lhs, rhs in
                compareDates(lhs.summary.updatedAt, rhs.summary.updatedAt, lhs.idKey, rhs.idKey, descending: descending)
            }.map(\.summary)
        case .createdAt:
            return matched.sorted { lhs, rhs in
                compareDates(lhs.summary.createdAt, rhs.summary.createdAt, lhs.idKey, rhs.idKey, descending: descending)
            }.map(\.summary)
        case .rating:
            return matched.sorted { lhs, rhs in
                compareRatingsNullsLast(lhs.summary.rating, rhs.summary.rating, lhs.idKey, rhs.idKey, descending: descending)
            }.map(\.summary)
        case .random:
            fatalError("random ordering is server-only; routing must not send it here")
        case .calories:
            // Server-only too (the cache has no calories), but routing a
            // calorie sort here shouldn't crash: keep the browse default.
            return matched.sorted { lhs, rhs in
                compareDates(lhs.summary.updatedAt, rhs.summary.updatedAt, lhs.idKey, rhs.idKey, descending: true)
            }.map(\.summary)
        }
    }

    /// `execute` for callers holding raw documents rather than a prepared
    /// corpus.
    static func execute(
        documents: [CachedRecipeSearchDocument],
        parsed: ParsedSearchQuery,
        sortBy: SortBy?,
        sortDir: Direction?
    ) -> [RecipeSummary] {
        execute(corpus: RecipeSearchCorpus(documents: documents), parsed: parsed, sortBy: sortBy, sortDir: sortDir)
    }

    private static func rankedByRelevance(
        _ matched: [RecipeSearchCorpus.Entry],
        texts: [RecipeSearchText],
        tokens: [[UInt8]]
    ) -> [RecipeSummary] {
        // Without text terms every score is zero and `texts` is empty.
        let scored = matched.enumerated().map { index, entry in
            (score: tokens.isEmpty ? 0 : relevanceScore(tokens: tokens, text: texts[index].fields), entry: entry)
        }
        return scored.sorted { lhs, rhs in
            if lhs.score != rhs.score {
                return lhs.score > rhs.score
            }
            if lhs.entry.summary.updatedAt != rhs.entry.summary.updatedAt {
                return lhs.entry.summary.updatedAt > rhs.entry.summary.updatedAt
            }
            return lhs.entry.idKey < rhs.entry.idKey
        }.map(\.entry.summary)
    }

    private static func compareDates(
        _ lhs: Date, _ rhs: Date, _ lhsID: String, _ rhsID: String, descending: Bool
    ) -> Bool {
        if lhs == rhs {
            return lhsID < rhsID
        }
        return descending ? lhs > rhs : lhs < rhs
    }

    /// Mirror of the server's `rating (a|de)sc NULLS LAST, id asc`.
    private static func compareRatingsNullsLast(
        _ lhs: Int?, _ rhs: Int?, _ lhsID: String, _ rhsID: String, descending: Bool
    ) -> Bool {
        switch (lhs, rhs) {
        case let (lhs?, rhs?) where lhs == rhs:
            return lhsID < rhsID
        case let (lhs?, rhs?):
            return descending ? lhs > rhs : lhs < rhs
        case (_?, nil):
            return true
        case (nil, _?):
            return false
        case (nil, nil):
            return lhsID < rhsID
        }
    }

    // MARK: - Shared helpers

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

// Kept outside the enum body to stay within its length limit.
extension RecipeSearchSupport {
    /// Mirror of the server's `parse_calorie_filter`: `300..600`, `300..`,
    /// `..600`, `>300` or `<600`, all inclusive. A bound is plain digits with
    /// an optional decimal part. Pinned by shared-test-vectors/calorie-filter.json.
    private static func parseCalorieFilter(_ expression: String, into result: inout ParsedSearchQuery) {
        func digits(_ part: Substring) -> Bool {
            !part.isEmpty && part.allSatisfy { $0.isASCII && $0.isNumber }
        }
        func bound(_ text: Substring) -> Double? {
            let parts = text.split(separator: ".", maxSplits: 1, omittingEmptySubsequences: false)
            guard parts.allSatisfy(digits) else {
                return nil
            }
            return Double(text)
        }
        if let dotsRange = expression.range(of: "..") {
            result.caloriesMin = bound(expression[..<dotsRange.lowerBound])
            result.caloriesMax = bound(expression[dotsRange.upperBound...])
        } else if expression.hasPrefix(">") {
            result.caloriesMin = bound(expression.dropFirst())
        } else if expression.hasPrefix("<") {
            result.caloriesMax = bound(expression.dropFirst())
        }
    }
}
