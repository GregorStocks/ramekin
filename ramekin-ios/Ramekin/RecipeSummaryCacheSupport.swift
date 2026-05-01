import Foundation

enum RecipeSummaryCacheSupport {
    static func canServeFromCache(filterState: RecipeListFilterState, sortOrder: RecipeSortOrder) -> Bool {
        filterState.searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && RecipeListFilterSupport.normalizedSource(filterState.source).isEmpty
            && sortOrder != .random
    }

    static func filteredAndSorted(
        _ recipes: [RecipeSummary],
        filterState: RecipeListFilterState,
        sortOrder: RecipeSortOrder
    ) -> [RecipeSummary] {
        let filtered = recipes.filter { recipe in
            matchesSearch(recipe, searchText: filterState.searchText)
                && matchesTags(recipe, selectedTags: filterState.selectedTags)
                && matchesPhotoFilter(recipe, photoFilter: filterState.photoFilter)
                && matchesCreatedDates(
                    recipe,
                    createdAfter: filterState.createdAfter,
                    createdBefore: filterState.createdBefore
                )
        }

        return filtered.sorted { lhs, rhs in
            switch sortOrder {
            case .newest:
                return compareDatesDescending(lhs.updatedAt, rhs.updatedAt, lhs.id, rhs.id)
            case .oldest:
                return compareDatesAscending(lhs.updatedAt, rhs.updatedAt, lhs.id, rhs.id)
            case .rating:
                return compareOptionalIntsDescending(lhs.rating, rhs.rating, lhs.id, rhs.id)
            case .title:
                let titleCompare = lhs.title.localizedCaseInsensitiveCompare(rhs.title)
                if titleCompare == .orderedSame {
                    return lhs.id.uuidString < rhs.id.uuidString
                }
                return titleCompare == .orderedAscending
            case .created:
                return compareDatesDescending(lhs.createdAt, rhs.createdAt, lhs.id, rhs.id)
            case .random:
                return lhs.id.uuidString < rhs.id.uuidString
            }
        }
    }

    private static func matchesSearch(_ recipe: RecipeSummary, searchText: String) -> Bool {
        let tokens = searchText
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .split(whereSeparator: \.isWhitespace)
            .map(String.init)
        guard !tokens.isEmpty else { return true }

        let searchableFields = [recipe.title, recipe.description ?? ""] + recipe.tags
        return tokens.allSatisfy { token in
            searchableFields.contains { field in
                field.localizedCaseInsensitiveContains(token)
            }
        }
    }

    private static func matchesTags(_ recipe: RecipeSummary, selectedTags: Set<String>) -> Bool {
        selectedTags.isSubset(of: Set(recipe.tags))
    }

    private static func matchesPhotoFilter(_ recipe: RecipeSummary, photoFilter: PhotoFilter) -> Bool {
        switch photoFilter {
        case .any:
            return true
        case .hasPhotos:
            return recipe.thumbnailPhotoId != nil
        case .noPhotos:
            return recipe.thumbnailPhotoId == nil
        }
    }

    private static func matchesCreatedDates(
        _ recipe: RecipeSummary,
        createdAfter: String,
        createdBefore: String
    ) -> Bool {
        let calendar = Calendar.autoupdatingCurrent
        if let after = RecipeListFilterSupport.date(from: createdAfter),
           calendar.startOfDay(for: recipe.createdAt) < calendar.startOfDay(for: after) {
            return false
        }
        if let before = RecipeListFilterSupport.date(from: createdBefore),
           calendar.startOfDay(for: recipe.createdAt) > calendar.startOfDay(for: before) {
            return false
        }
        return true
    }

    private static func compareDatesDescending(_ lhs: Date, _ rhs: Date, _ lhsID: UUID, _ rhsID: UUID) -> Bool {
        if lhs == rhs {
            return lhsID.uuidString < rhsID.uuidString
        }
        return lhs > rhs
    }

    private static func compareDatesAscending(_ lhs: Date, _ rhs: Date, _ lhsID: UUID, _ rhsID: UUID) -> Bool {
        if lhs == rhs {
            return lhsID.uuidString < rhsID.uuidString
        }
        return lhs < rhs
    }

    private static func compareOptionalIntsDescending(_ lhs: Int?, _ rhs: Int?, _ lhsID: UUID, _ rhsID: UUID) -> Bool {
        switch (lhs, rhs) {
        case let (lhs?, rhs?) where lhs == rhs:
            return lhsID.uuidString < rhsID.uuidString
        case let (lhs?, rhs?):
            return lhs > rhs
        case (_?, nil):
            return true
        case (nil, _?):
            return false
        case (nil, nil):
            return lhsID.uuidString < rhsID.uuidString
        }
    }
}
