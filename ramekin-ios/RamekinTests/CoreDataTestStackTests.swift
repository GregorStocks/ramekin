import CoreData
import XCTest

@testable import Ramekin

/// Guards the invariant that makes `CoreDataTestStack` safe to call from every suite: no matter how
/// many stacks exist, exactly one `NSEntityDescription` claims each managed-object subclass. When
/// that breaks, Core Data logs an ambiguous-entity warning and `ShoppingItem(context:)` inserts
/// against whichever model it happens to pick.
final class CoreDataTestStackTests: XCTestCase {
    private let entityNames = ["CachedRecipe", "ShoppingItem"]

    func testEveryContainerSharesOneEntityDescriptionPerSubclass() {
        let first = CoreDataTestStack.makeContainer()
        let second = CoreDataTestStack.makeContainer()

        XCTAssertTrue(first.managedObjectModel === second.managedObjectModel)

        for name in entityNames {
            guard let firstEntity = first.managedObjectModel.entitiesByName[name],
                let secondEntity = second.managedObjectModel.entitiesByName[name]
            else {
                return XCTFail("Model is missing the \(name) entity")
            }
            XCTAssertTrue(
                firstEntity === secondEntity,
                "\(name) is claimed by two entity descriptions, so +entity is unreliable"
            )
        }
    }

    func testInsertedObjectsUseTheirOwnContextsEntities() throws {
        // Load an extra container first: with per-container models this is the stale one that
        // `+entity` could resolve against, which is exactly the failure from PR #652.
        _ = CoreDataTestStack.makeContainer()
        let container = CoreDataTestStack.makeContainer()
        let context = container.viewContext

        let item = ShoppingItem(context: context)
        let recipe = CachedRecipe(context: context)

        XCTAssertTrue(item.entity === container.managedObjectModel.entitiesByName["ShoppingItem"])
        XCTAssertTrue(recipe.entity === container.managedObjectModel.entitiesByName["CachedRecipe"])
        XCTAssertTrue(item.managedObjectContext === context)
        XCTAssertTrue(recipe.managedObjectContext === context)
    }

    func testStacksDoNotShareStoredItems() throws {
        let firstContext = CoreDataTestStack.makeContext()
        let secondContext = CoreDataTestStack.makeContext()
        let accountKey = "https://example.test|chef"

        _ = ShoppingItem.create(in: firstContext, accountKey: accountKey, item: "Milk")
        try firstContext.save()

        XCTAssertEqual(
            try firstContext.count(for: ShoppingItem.fetchActiveItems(accountKey: accountKey)),
            1
        )
        XCTAssertEqual(
            try secondContext.count(for: ShoppingItem.fetchActiveItems(accountKey: accountKey)),
            0
        )
    }
}
