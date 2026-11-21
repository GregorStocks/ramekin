import Foundation

/// One recipe's scorer fields, normalized with the search contract
/// (`SearchNormalizationSupport.normalizedUTF8`).
struct NormalizedSearchText {
    let title: [UInt8]
    let description: [UInt8]?
    let tags: [[UInt8]]
    /// One entry per ingredient.
    let ingredients: [[UInt8]]
    let instructions: [UInt8]
    let notes: [UInt8]?

    init(_ document: RecipeSearchSupport.ScoringDocument) {
        let normalize = SearchNormalizationSupport.normalizedUTF8
        title = normalize(document.title)
        description = document.description.map(normalize)
        tags = document.tags.map(normalize)
        ingredients = document.ingredients.map(normalize)
        instructions = normalize(document.instructions)
        notes = document.notes.map(normalize)
    }
}

/// Everything a text query reads from one recipe.
struct RecipeSearchText {
    let fields: NormalizedSearchText
    /// The normalized ingredient match text. Membership matches it; scoring
    /// uses `fields.ingredients` instead.
    let ingredientMatch: [UInt8]

    init(_ document: CachedRecipeSearchDocument) {
        fields = NormalizedSearchText(RecipeSearchSupport.scoringDocument(for: document))
        ingredientMatch = SearchNormalizationSupport.normalizedUTF8(document.ingredientMatchText)
    }
}

/// The cached cookbook prepared for searching. Normalizing a thousand
/// recipes' text costs far more than scanning it, so it happens once per
/// corpus — the first time a text query needs it, or ahead of that through
/// `prepareSearchTexts` — and every query after only scans. Browsing never
/// pays for it.
struct RecipeSearchCorpus {
    struct Entry {
        let summary: RecipeSummary
        /// `summary.id.uuidString`, the tie-break every ordering ends on.
        let idKey: String
        /// The tags under `citextFold`, the form `tag:` filters compare.
        let foldedTags: [String]
        /// The normalized source name, which `source:` matches.
        let source: [UInt8]?
        let kcalPerServing: Double?
        let photos: [SyncPhoto]

        init(_ document: CachedRecipeSearchDocument) {
            summary = document.summary
            idKey = document.summary.id.uuidString
            foldedTags = document.summary.tags.map(SearchNormalizationSupport.citextFold)
            source = document.sourceName.map(SearchNormalizationSupport.normalizedUTF8)
            kcalPerServing = document.kcalPerServing
            photos = document.photos
        }
    }

    let entries: [Entry]
    /// Changes whenever the corpus is rebuilt, so a ranking can be reused
    /// for as long as the cache hands back the same corpus.
    let version = UUID()
    private let texts: LazySearchTexts

    var isEmpty: Bool { entries.isEmpty }

    init(documents: [CachedRecipeSearchDocument]) {
        entries = documents.map(Entry.init)
        texts = LazySearchTexts(documents)
    }

    /// The normalized text of each recipe, parallel to `entries`.
    func searchTexts() -> [RecipeSearchText] {
        texts.value()
    }

    /// Normalizes the corpus ahead of the first text query. Safe to call
    /// from any thread, alongside searches.
    func prepareSearchTexts() {
        _ = texts.value()
    }
}

/// Normalizes the corpus on first use, then drops the raw documents.
private final class LazySearchTexts: @unchecked Sendable {
    private let lock = NSLock()
    private var documents: [CachedRecipeSearchDocument]
    private var texts: [RecipeSearchText]?

    init(_ documents: [CachedRecipeSearchDocument]) {
        self.documents = documents
    }

    func value() -> [RecipeSearchText] {
        lock.lock()
        defer { lock.unlock() }
        if let texts {
            return texts
        }
        let built = documents.map(RecipeSearchText.init)
        texts = built
        documents = []
        return built
    }
}

/// A parsed query with everything that is compared per recipe computed once.
struct PreparedSearchQuery {
    let parsed: ParsedSearchQuery
    /// The text terms, normalized.
    let tokens: [[UInt8]]
    let foldedTags: [String]
    /// The `source:` value, normalized.
    let source: [UInt8]?
    /// Start of the UTC day each created-date bound names.
    let createdAfterDay: Date?
    let createdBeforeDay: Date?

    init(_ parsed: ParsedSearchQuery) {
        self.parsed = parsed
        tokens = parsed.textTokens.map(SearchNormalizationSupport.normalizedUTF8)
        foldedTags = parsed.tags.map(SearchNormalizationSupport.citextFold)
        source = parsed.source.map(SearchNormalizationSupport.normalizedUTF8)
        createdAfterDay = RecipeSearchSupport.utcDay(parsed.createdAfter)
        createdBeforeDay = RecipeSearchSupport.utcDay(parsed.createdBefore)
    }
}
