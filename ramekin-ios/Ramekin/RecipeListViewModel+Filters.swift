import Foundation

/// The list filter state as one value, and the keys it persists under.
extension RecipeListViewModel {
    var hasActiveFilters: Bool {
        currentFilterState.hasAnyFilters
    }

    var currentFilterState: RecipeListFilterState {
        RecipeListFilterState(
            searchText: searchText,
            selectedTags: selectedTags,
            photoFilter: photoFilter,
            source: sourceFilter,
            createdAfter: createdAfterFilter,
            createdBefore: createdBeforeFilter,
            photoSizeThreshold: RecipeListFilterSupport.numericThreshold(from: photoSizeFilter),
            photoDimensionThreshold: RecipeListFilterSupport.numericThreshold(from: photoDimensionFilter),
            caloriesMin: caloriesMinFilter,
            caloriesMax: caloriesMaxFilter
        )
    }

    var advancedFilterLabel: String {
        RecipeListFilterSupport.advancedFilterLabel(for: currentFilterState) ?? "Filters"
    }

    static let sortOrderKey = "recipeSortOrder"
    static let photoFilterKey = "recipePhotoFilter"
    static let sourceFilterKey = "recipeSourceFilter"
    static let createdAfterFilterKey = "recipeCreatedAfterFilter"
    static let createdBeforeFilterKey = "recipeCreatedBeforeFilter"
    static let photoSizeFilterKey = "recipePhotoSizeFilter"
    static let photoDimensionFilterKey = "recipePhotoDimensionFilter"
    static let caloriesMinFilterKey = "recipeCaloriesMinFilter"
    static let caloriesMaxFilterKey = "recipeCaloriesMaxFilter"
}
