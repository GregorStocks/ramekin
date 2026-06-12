import Foundation

enum ShoppingListGroupingSupport {
    /// Orders the categories present in the list by the server-provided
    /// category order. Categories the order doesn't mention (items created
    /// offline before the first sync has delivered the order) are appended
    /// alphabetically so their items are never hidden.
    static func orderedCategories(present: Set<String>, categoryOrder: [String]) -> [String] {
        let known = categoryOrder.filter { present.contains($0) }
        let unknown = present.subtracting(categoryOrder).sorted()
        return known + unknown
    }
}
