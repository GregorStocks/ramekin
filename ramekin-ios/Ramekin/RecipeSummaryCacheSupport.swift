import Foundation

enum RecipeSummaryCacheSupport {
    /// The cached recipes the current filter+sort should display, in order —
    /// the local mirror of the exact request the app would otherwise send the
    /// server. The whole filter state collapses into the same query string
    /// `buildQuery` sends over the network, and text queries rank by
    /// relevance exactly like the server default (the app never sends an
    /// explicit sort with a text query).
    static func visibleRecipes(
        corpus: RecipeSearchCorpus,
        filterState: RecipeListFilterState,
        sortOrder: RecipeSortOrder
    ) -> [RecipeSummary] {
        let parsed = parsedQuery(for: filterState)
        let hasText = !parsed.textTokens.isEmpty
        return RecipeSearchSupport.execute(
            corpus: corpus,
            parsed: parsed,
            sortBy: hasText ? nil : sortOrder.sortBy,
            sortDir: hasText ? nil : sortOrder.sortDir
        )
    }

    static func visibleRecipes(
        documents: [CachedRecipeSearchDocument],
        filterState: RecipeListFilterState,
        sortOrder: RecipeSortOrder
    ) -> [RecipeSummary] {
        visibleRecipes(corpus: RecipeSearchCorpus(documents: documents), filterState: filterState, sortOrder: sortOrder)
    }

    /// The same query string the app would send the server, parsed the same
    /// way the server parses it.
    private static func parsedQuery(for filterState: RecipeListFilterState) -> ParsedSearchQuery {
        RecipeSearchSupport.parse(RecipeListFilterSupport.buildQuery(from: filterState) ?? "")
    }
}
