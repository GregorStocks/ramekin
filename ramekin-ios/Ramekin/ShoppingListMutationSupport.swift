import CoreData
import Foundation

enum ShoppingListStoreError: LocalizedError {
    case saveFailed(underlying: Error)

    var errorDescription: String? {
        switch self {
        case .saveFailed:
            return "Failed to save shopping list changes."
        }
    }
}

enum ShoppingListMutationSupport {
    static func updateCategoryOverride(_ item: ShoppingItem, categoryOverride: String?) {
        item.categoryOverride = categoryOverride
        item.clearCategoryOverride = categoryOverride == nil && item.syncStatusEnum != .pendingCreate
        item.category = categoryOverride ?? item.computedCategory
        item.markUpdated()
    }

    static func addItemsFromRecipe(
        ingredients: [(name: String, amount: String?)],
        recipe: (id: UUID, title: String),
        accountKey: String,
        context: NSManagedObjectContext,
        save: () throws -> Void
    ) throws {
        do {
            let currentItems = try context.fetch(ShoppingItem.fetchActiveItems(accountKey: accountKey))
            var maxSort = currentItems.map(\.sortOrder).max() ?? -1
            for ingredient in ingredients {
                maxSort += 1
                _ = ShoppingItem.create(
                    in: context,
                    accountKey: accountKey,
                    item: ingredient.name,
                    amount: ingredient.amount,
                    sourceRecipeId: recipe.id,
                    sourceRecipeTitle: recipe.title,
                    sortOrder: maxSort
                )
            }

            try save()
        } catch {
            context.rollback()
            throw ShoppingListStoreError.saveFailed(underlying: error)
        }
    }
}
