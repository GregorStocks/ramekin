import CoreData
import XCTest
@testable import Ramekin

final class ShoppingListSyncSupportTests: XCTestCase {
    func testReconcileMarksUnchangedSuccessfulUpdateAsSynced() throws {
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
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
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
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
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
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
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
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

    private var accountKey: String { "https://example.test|chef" }
}
