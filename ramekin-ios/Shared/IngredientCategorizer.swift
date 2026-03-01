import Foundation

/// Maps ingredient names to grocery store aisle categories based on keyword matching.
/// Mirrors the server-side categorizer in `ramekin-core/src/ingredient_categorizer.rs`.
enum IngredientCategorizer {
    /// Category display order, matching ShoppingListView.categoryOrder
    static let categoryOrder = [
        "Produce",
        "Meat & Seafood",
        "Dairy & Eggs",
        "Cheese",
        "Bakery & Bread",
        "Frozen",
        "Pasta & Rice",
        "Canned Goods",
        "Baking",
        "Spices & Seasonings",
        "Condiments & Sauces",
        "Oils & Vinegars",
        "Nuts & Dried Fruit",
        "Beverages",
        "Snacks",
        "Other",
    ]

    /// Keyword-to-category map, sorted by keyword length descending for specificity.
    /// Loaded lazily from the bundled ingredients.json.
    private static let ingredientMap: [(keyword: String, category: String)] = {
        guard let url = Bundle.main.url(forResource: "ingredients", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let parsed = try? JSONDecoder().decode(IngredientsData.self, from: data)
        else {
            return []
        }
        // Sort by keyword length descending (longer = more specific matches first),
        // then alphabetically for deterministic ordering. Mirrors the Rust implementation.
        return parsed.categories
            .sorted { a, b in
                if a.key.count != b.key.count {
                    return a.key.count > b.key.count
                }
                return a.key < b.key
            }
            .map { (keyword: $0.key, category: $0.value) }
    }()

    /// Categorize an ingredient by name.
    /// Returns the category name, or "Other" if no match is found.
    static func categorize(_ item: String) -> String {
        let lower = item.lowercased()
        for entry in ingredientMap {
            if lower.contains(entry.keyword) {
                return entry.category
            }
        }
        return "Other"
    }

    /// Returns the sort index for a category (lower = earlier in the store).
    static func categoryIndex(_ category: String) -> Int {
        categoryOrder.firstIndex(of: category) ?? categoryOrder.count - 1
    }
}

private struct IngredientsData: Decodable {
    let categories: [String: String]
}
