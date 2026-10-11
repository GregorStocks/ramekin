import CoreData
import XCTest
@testable import Ramekin

final class ShoppingListSyncSupportTests: XCTestCase {
    func testReconcileMarksUnchangedSuccessfulUpdateAsSynced() throws {
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
        item.syncStatusEnum = .pendingUpdate
        item.updatedAt = Date(timeIntervalSince1970: 100)

        let outcome = ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 4,
            success: true,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        XCTAssertEqual(outcome, .kept)
        XCTAssertEqual(item.syncStatusEnum, .synced)
        XCTAssertEqual(item.serverVersion, 4)
    }

    func testReconcileKeepsPendingCreateModifiedDuringSyncAsPendingUpdate() throws {
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
        item.updatedAt = Date(timeIntervalSince1970: 250)

        let outcome = ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 7,
            success: true,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        XCTAssertEqual(outcome, .kept)
        XCTAssertEqual(item.syncStatusEnum, .pendingUpdate)
        XCTAssertEqual(item.serverVersion, 7)
    }

    func testReconcileTakesTheServerCopyWhenAnUpdateIsRejected() throws {
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
        item.syncStatusEnum = .pendingUpdate
        item.serverVersion = 3
        item.updatedAt = Date(timeIntervalSince1970: 100)

        let outcome = ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 5,
            success: false,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        // Synced at version 0, so the server's row in the same response
        // replaces the local copy instead of the local copy being resent.
        XCTAssertEqual(outcome, .kept)
        XCTAssertEqual(item.syncStatusEnum, .synced)
        XCTAssertEqual(item.serverVersion, 0)
    }

    func testReconcileKeepsAnEditMadeDuringARejectedSync() throws {
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
        item.syncStatusEnum = .pendingUpdate
        item.serverVersion = 3
        item.updatedAt = Date(timeIntervalSince1970: 300)

        let outcome = ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 5,
            success: false,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        XCTAssertEqual(outcome, .kept)
        XCTAssertEqual(item.syncStatusEnum, .pendingUpdate)
        XCTAssertEqual(item.serverVersion, 5)
    }

    func testReconcileReportsAnItemTheServerNoLongerHas() throws {
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", sortOrder: 0)
        item.syncStatusEnum = .pendingUpdate
        item.serverVersion = 2
        item.updatedAt = Date(timeIntervalSince1970: 100)

        let outcome = ShoppingListSyncSupport.reconcileSyncedItem(
            item,
            version: 0,
            success: false,
            syncStartedAt: Date(timeIntervalSince1970: 200)
        )

        XCTAssertEqual(outcome, .deletedOnServer)
    }

    private var accountKey: String { "https://example.test|chef" }
}
