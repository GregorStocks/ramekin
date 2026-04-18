import CoreData
import XCTest
@testable import Ramekin

final class ShoppingListSyncSupportTests: XCTestCase {
    func testReconcileMarksUnchangedSuccessfulUpdateAsSynced() throws {
        let context = makeInMemoryContainer().viewContext
        let item = ShoppingItem.create(in: context, item: "Milk", sortOrder: 0)
        item.syncStatusEnum = .pendingUpdate
        item.updatedAt = Date(timeIntervalSince1970: 100)

        ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 4,
            success: true,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        XCTAssertEqual(item.syncStatusEnum, .synced)
        XCTAssertEqual(item.serverVersion, 4)
    }

    func testReconcileKeepsPendingCreateModifiedDuringSyncAsPendingUpdate() throws {
        let context = makeInMemoryContainer().viewContext
        let item = ShoppingItem.create(in: context, item: "Milk", sortOrder: 0)
        item.updatedAt = Date(timeIntervalSince1970: 250)

        ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 7,
            success: true,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        XCTAssertEqual(item.syncStatusEnum, .pendingUpdate)
        XCTAssertEqual(item.serverVersion, 7)
    }

    func testReconcileRetainsPendingUpdateWhenServerRejectsWithNewVersion() throws {
        let context = makeInMemoryContainer().viewContext
        let item = ShoppingItem.create(in: context, item: "Milk", sortOrder: 0)
        item.syncStatusEnum = .pendingUpdate
        item.serverVersion = 3
        item.updatedAt = Date(timeIntervalSince1970: 100)

        ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 5,
            success: false,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        XCTAssertEqual(item.syncStatusEnum, .pendingUpdate)
        XCTAssertEqual(item.serverVersion, 5)
    }

    func testReconcileDoesNotClobberPendingItemWhenServerReturnsVersionZero() throws {
        let context = makeInMemoryContainer().viewContext
        let item = ShoppingItem.create(in: context, item: "Milk", sortOrder: 0)
        item.serverVersion = 2
        item.updatedAt = Date(timeIntervalSince1970: 300)

        ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 0,
            success: false,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        XCTAssertEqual(item.syncStatusEnum, .pendingCreate)
        XCTAssertEqual(item.serverVersion, 2)
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
