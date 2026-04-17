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
    static func addItemsFromRecipe(
        ingredients: [(name: String, amount: String?)],
        recipeId: UUID,
        recipeTitle: String,
        context: NSManagedObjectContext,
        save: () throws -> Void
    ) throws {
        do {
            let currentItems = try context.fetch(ShoppingItem.fetchActiveItems())
            var maxSort = currentItems.map(\.sortOrder).max() ?? -1
            for ingredient in ingredients {
                maxSort += 1
                _ = ShoppingItem.create(
                    in: context,
                    item: ingredient.name,
                    amount: ingredient.amount,
                    sourceRecipeId: recipeId,
                    sourceRecipeTitle: recipeTitle,
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
