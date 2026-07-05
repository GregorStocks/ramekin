import CoreData
import XCTest
@testable import Ramekin

final class ShoppingListMutationSupportTests: XCTestCase {
    func testUpdateCategoryOverrideSetsCategoryAndMarksSyncedItemPending() throws {
        let context = makeInMemoryContainer().viewContext
        let item = ShoppingItem.create(in: context, item: "Milk", sortOrder: 0)
        item.markSynced(serverVersion: 3)

        ShoppingListMutationSupport.updateCategoryOverride(item, categoryOverride: "Produce")

        XCTAssertEqual(item.categoryOverride, "Produce")
        XCTAssertEqual(item.category, "Produce")
        XCTAssertFalse(item.clearCategoryOverride)
        XCTAssertEqual(item.syncStatusEnum, .pendingUpdate)
    }

    func testUpdateCategoryOverrideClearsSyncedItemWithDirtyFlag() throws {
        let context = makeInMemoryContainer().viewContext
        let item = ShoppingItem.create(in: context, item: "Milk", sortOrder: 0)
        item.categoryOverride = "Produce"
        item.category = "Produce"
        item.markSynced(serverVersion: 3)

        ShoppingListMutationSupport.updateCategoryOverride(item, categoryOverride: nil)

        XCTAssertNil(item.categoryOverride)
        XCTAssertNil(item.category)
        XCTAssertTrue(item.clearCategoryOverride)
        XCTAssertEqual(item.syncStatusEnum, .pendingUpdate)
    }

    func testUpdateCategoryOverrideClearsPendingCreateWithoutDirtyFlag() throws {
        let context = makeInMemoryContainer().viewContext
        let item = ShoppingItem.create(in: context, item: "Milk", sortOrder: 0)
        item.categoryOverride = "Produce"
        item.category = "Produce"

        ShoppingListMutationSupport.updateCategoryOverride(item, categoryOverride: nil)

        XCTAssertNil(item.categoryOverride)
        XCTAssertNil(item.category)
        XCTAssertFalse(item.clearCategoryOverride)
        XCTAssertEqual(item.syncStatusEnum, .pendingCreate)
    }

    func testAddItemsFromRecipeAssignsSequentialSortOrderAndMetadata() throws {
        let container = makeInMemoryContainer()
        let context = container.viewContext
        let recipeId = UUID()

        _ = ShoppingItem.create(
            in: context,
            item: "Existing",
            amount: nil,
            sourceRecipeId: nil,
            sourceRecipeTitle: nil,
            sortOrder: 4
        )

        try ShoppingListMutationSupport.addItemsFromRecipe(
            ingredients: [
                (name: "Apples", amount: "2"),
                (name: "Flour", amount: "1 cup")
            ],
            recipeId: recipeId,
            recipeTitle: "Pie",
            context: context,
            save: {
                try context.save()
            }
        )

        let items = try context.fetch(ShoppingItem.fetchActiveItems())
        XCTAssertEqual(items.map { $0.item ?? "" }, ["Existing", "Apples", "Flour"])
        XCTAssertEqual(items.map(\.sortOrder), [4, 5, 6])
        XCTAssertEqual(items[1].sourceRecipeId, recipeId)
        XCTAssertEqual(items[1].sourceRecipeTitle, "Pie")
        XCTAssertEqual(items[2].amount, "1 cup")
    }

    func testAddItemsFromRecipeRollsBackAndThrowsWhenSaveFails() throws {
        let container = makeInMemoryContainer()
        let context = container.viewContext
        let recipeId = UUID()
        let underlyingSaveError = NSError(
            domain: "ShoppingListMutationSupportTests",
            code: 42,
            userInfo: [NSLocalizedDescriptionKey: "disk full"]
        )

        XCTAssertThrowsError(
            try ShoppingListMutationSupport.addItemsFromRecipe(
                ingredients: [(name: "Apples", amount: "2")],
                recipeId: recipeId,
                recipeTitle: "Pie",
                context: context,
                save: {
                    throw underlyingSaveError
                }
            )
        ) { error in
            guard let shoppingListError = error as? ShoppingListStoreError else {
                return XCTFail("Expected ShoppingListStoreError, got \(error)")
            }

            guard case .saveFailed(let underlying) = shoppingListError else {
                return XCTFail("Expected saveFailed error, got \(error)")
            }

            XCTAssertEqual((underlying as NSError).domain, underlyingSaveError.domain)
            XCTAssertEqual(error.localizedDescription, "Failed to save shopping list changes.")
        }

        XCTAssertEqual(try context.count(for: ShoppingItem.fetchActiveItems()), 0)
        XCTAssertFalse(context.hasChanges)
    }

    private func makeInMemoryContainer() -> NSPersistentContainer {
        let container = NSPersistentContainer(name: "Ramekin")
        let description = NSPersistentStoreDescription()
        description.type = NSInMemoryStoreType
        container.persistentStoreDescriptions = [description]

        let loaded = expectation(description: "persistent store loaded")
        container.loadPersistentStores { _, error in
            XCTAssertNil(error)
            loaded.fulfill()
        }
        wait(for: [loaded], timeout: 5)
        return container
    }
}
