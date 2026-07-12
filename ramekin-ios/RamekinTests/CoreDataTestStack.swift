import CoreData
import XCTest

@testable import Ramekin

/// Builds Core Data stacks backed by an in-memory store.
///
/// Every container comes from `CoreDataStack.makeContainer()`, so all of them share one
/// `NSManagedObjectModel`. That keeps a single entity description per managed-object subclass no
/// matter how many stacks a test run creates, which is what makes `ShoppingItem(context:)` and
/// `CachedRecipe(context:)` insert into the context they were handed.
enum CoreDataTestStack {
    static func makeContainer(
        file: StaticString = #filePath,
        line: UInt = #line
    ) -> NSPersistentContainer {
        let container = CoreDataStack.makeContainer()
        let description = NSPersistentStoreDescription()
        description.type = NSInMemoryStoreType
        description.shouldAddStoreAsynchronously = false
        container.persistentStoreDescriptions = [description]

        var loadError: Error?
        container.loadPersistentStores { loadError = $1 }
        XCTAssertNil(loadError, "Failed to load in-memory store", file: file, line: line)

        return container
    }

    static func makeStack(file: StaticString = #filePath, line: UInt = #line) -> CoreDataStack {
        CoreDataStack(container: makeContainer(file: file, line: line))
    }

    static func makeContext(
        file: StaticString = #filePath,
        line: UInt = #line
    ) -> NSManagedObjectContext {
        makeContainer(file: file, line: line).viewContext
    }
}
