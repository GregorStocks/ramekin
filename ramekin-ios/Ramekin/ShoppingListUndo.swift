import CoreData
import Foundation

/// Deleted shopping items the list offers to bring back, as they were when
/// deleted, so a row whose delete already synced can be shown again at once.
struct ShoppingListUndo: Identifiable, Equatable {
    struct Item: Equatable {
        let id: UUID
        let item: String?
        let amount: String?
        let note: String?
        let isChecked: Bool
        let sortOrder: Int32
        let categoryOverride: String?
        let category: String?
        let computedCategory: String?
        let sourceRecipeId: UUID?
        let sourceRecipeTitle: String?
        let changeSeq: Int64

        init(_ item: ShoppingItem) {
            id = item.id ?? UUID()
            self.item = item.item
            amount = item.amount
            note = item.note
            isChecked = item.isChecked
            sortOrder = item.sortOrder
            categoryOverride = item.categoryOverride
            category = item.category
            computedCategory = item.computedCategory
            sourceRecipeId = item.sourceRecipeId
            sourceRecipeTitle = item.sourceRecipeTitle
            changeSeq = item.changeSeq
        }
    }

    let id = UUID()
    let message: String
    let items: [Item]
}
