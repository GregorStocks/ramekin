import Foundation

enum RecipeSummaryCacheSupport {
    /// Whether the current filter+sort can be answered from the local cache
    /// with server-identical results. Queries needing `source:`,
    /// `photo_size:`, or `photo_dim:` stay on the server because the synced
    /// recipes carry neither the source name nor detailed photo metadata.
    /// Text queries run locally regardless of the selected browse sort —
    /// both sides rank them by relevance — so only random *browsing* (no
    /// text terms) remains server-only.
    static func canServeFromCache(filterState: RecipeListFilterState, sortOrder: RecipeSortOrder) -> Bool {
        let parsed = parsedQuery(for: filterState)
        if parsed.requiresServer {
            return false
        }
        return !parsed.textTokens.isEmpty || sortOrder != .random
    }

    /// The cached recipes the current filter+sort should display, in order —
    /// the local mirror of the exact request the app would otherwise send the
    /// server. The whole filter state collapses into the same query string
    /// `buildQuery` sends over the network, and text queries rank by
    /// relevance exactly like the server default (the app never sends an
    /// explicit sort with a text query).
    static func visibleRecipes(
        documents: [CachedRecipeSearchDocument],
        filterState: RecipeListFilterState,
        sortOrder: RecipeSortOrder
    ) -> [RecipeSummary] {
        let parsed = parsedQuery(for: filterState)
        let hasText = !parsed.textTokens.isEmpty
        return RecipeSearchSupport.execute(
            documents: documents,
            parsed: parsed,
            sortBy: hasText ? nil : sortOrder.sortBy,
            sortDir: hasText ? nil : sortOrder.sortDir
        )
    }

    /// The same query string the app would send the server, parsed the same
    /// way the server parses it.
    private static func parsedQuery(for filterState: RecipeListFilterState) -> ParsedSearchQuery {
        RecipeSearchSupport.parse(RecipeListFilterSupport.buildQuery(from: filterState) ?? "")
    }
}
