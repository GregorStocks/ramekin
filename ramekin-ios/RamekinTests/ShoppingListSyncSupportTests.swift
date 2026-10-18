import CoreData
import XCTest
@testable import Ramekin

final class ShoppingListSyncSupportTests: XCTestCase {
    func testAnUpdateSendsOnlyItsDirtyFieldsAndBlankClears() throws {
        let context = CoreDataTestStack.makeContext()
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Milk", amount: "1 gallon", sortOrder: 0)
        item.markSynced()
        item.amount = nil
        item.isChecked = true
        item.markUpdated(["amount", "is_checked"])

        let outbox = ShoppingListSyncSupport.outbox(for: [item])

        let operation = try XCTUnwrap(outbox.ops.first)
        XCTAssertEqual(operation.op, .update)
        XCTAssertEqual(operation.fields?.amount, "")
        XCTAssertEqual(operation.fields?.isChecked, true)
        XCTAssertNil(operation.fields?.item)
        XCTAssertNil(operation.fields?.note)
        XCTAssertNil(operation.fields?.categoryOverride)
    }

    func testACreateSendsTheWholeItemUnderItsOwnId() throws {
        let context = CoreDataTestStack.makeContext()
        let recipeId = UUID()
        let item = ShoppingItem.create(
            in: context, accountKey: accountKey, item: "Flour", amount: "2 cups",
            sourceRecipeId: recipeId, sourceRecipeTitle: "Bread", sortOrder: 3
        )

        let operation = try XCTUnwrap(ShoppingListSyncSupport.outbox(for: [item]).ops.first)

        XCTAssertEqual(operation.op, .create)
        XCTAssertEqual(operation.id, item.id)
        XCTAssertEqual(operation.fields?.item, "Flour")
        XCTAssertEqual(operation.fields?.amount, "2 cups")
        XCTAssertEqual(operation.fields?.sortOrder, 3)
        XCTAssertEqual(operation.fields?.sourceRecipeId, recipeId)
    }

    func testAppliedOpMarksTheItemSynced() throws {
        let context = CoreDataTestStack.makeContext()
        let item = syncedItem(in: context)
        item.isChecked = true
        item.markUpdated(["is_checked"])
        let sent = try XCTUnwrap(ShoppingListSyncSupport.outbox(for: [item]).sent[XCTUnwrap(item.id)])

        XCTAssertEqual(ShoppingListSyncSupport.settle(item, result: .applied, sent: sent), .kept)
        XCTAssertEqual(item.syncStatusEnum, .synced)
        XCTAssertTrue(item.dirtyFieldSet.isEmpty)
    }

    func testAFieldEditedAgainMidSyncStaysDirty() throws {
        let context = CoreDataTestStack.makeContext()
        let item = syncedItem(in: context)
        item.note = "2%"
        item.markUpdated(["note"])
        let sent = try XCTUnwrap(ShoppingListSyncSupport.outbox(for: [item]).sent[XCTUnwrap(item.id)])
        // While the sync is in flight: the note changes again, and the item
        // gets checked off.
        item.note = "whole"
        item.isChecked = true
        item.markUpdated(["note", "is_checked"])

        XCTAssertEqual(ShoppingListSyncSupport.settle(item, result: .applied, sent: sent), .kept)
        XCTAssertEqual(item.syncStatusEnum, .pendingUpdate)
        XCTAssertEqual(item.dirtyFieldSet, ["note", "is_checked"])
    }

    func testAnEchoOverwritesOnlyFieldsWithoutUnsentEdits() throws {
        let context = CoreDataTestStack.makeContext()
        let item = syncedItem(in: context)
        let id = try XCTUnwrap(item.id)
        item.isChecked = true
        item.markUpdated(["is_checked"])

        // Another device renamed it; this device's check-off isn't sent yet.
        ShoppingListSyncSupport.apply(
            response(changes: [change(id: id, item: "6 eggs", changeSeq: 9)]),
            accountKey: accountKey,
            in: context
        )

        XCTAssertEqual(item.item, "6 eggs")
        XCTAssertTrue(item.isChecked)
        XCTAssertEqual(item.changeSeq, 9)
        XCTAssertEqual(item.syncStatusEnum, .pendingUpdate)
    }

    func testAnOlderServerRowIsSkipped() throws {
        let context = CoreDataTestStack.makeContext()
        let item = syncedItem(in: context)
        item.changeSeq = 20

        ShoppingListSyncSupport.apply(
            response(changes: [change(id: try XCTUnwrap(item.id), item: "stale", changeSeq: 10)]),
            accountKey: accountKey,
            in: context
        )

        XCTAssertEqual(item.item, "Eggs")
    }

    func testGoneRemovesTheItem() throws {
        let context = CoreDataTestStack.makeContext()
        let item = syncedItem(in: context)
        item.markUpdated(["note"])
        let sent = try XCTUnwrap(ShoppingListSyncSupport.outbox(for: [item]).sent[XCTUnwrap(item.id)])

        XCTAssertEqual(ShoppingListSyncSupport.settle(item, result: .gone, sent: sent), .removed)
    }

    func testADeleteMadeMidSyncStaysPending() throws {
        let context = CoreDataTestStack.makeContext()
        let item = syncedItem(in: context)
        item.note = "2%"
        item.markUpdated(["note"])
        let sent = try XCTUnwrap(ShoppingListSyncSupport.outbox(for: [item]).sent[XCTUnwrap(item.id)])
        item.markDeleted()

        XCTAssertEqual(ShoppingListSyncSupport.settle(item, result: .applied, sent: sent), .kept)
        XCTAssertEqual(item.syncStatusEnum, .pendingDelete)
    }

    func testAPendingUpdateFromBeforeFieldTrackingSendsEveryField() throws {
        let context = CoreDataTestStack.makeContext()
        let item = syncedItem(in: context)
        item.syncStatusEnum = .pendingUpdate
        item.dirtyFields = nil

        XCTAssertEqual(item.dirtyFieldSet, Set(ShoppingItem.syncedFields))
    }

    private func syncedItem(in context: NSManagedObjectContext) -> ShoppingItem {
        let item = ShoppingItem.create(in: context, accountKey: accountKey, item: "Eggs", sortOrder: 0)
        item.markSynced()
        item.changeSeq = 5
        return item
    }

    private func change(id: UUID, item: String, changeSeq: Int64) -> SyncShoppingListItem {
        SyncShoppingListItem(
            category: "Dairy & Eggs", changeSeq: changeSeq, computedCategory: "Dairy & Eggs",
            id: id, isChecked: false, item: item, sortOrder: 0
        )
    }

    private func response(changes: [SyncShoppingListItem]) -> SyncShoppingListResponse {
        SyncShoppingListResponse(
            categoryOrder: [], changes: changes, cursor: 100, deleted: [], fullResync: false, results: []
        )
    }

    private var accountKey: String { "https://example.test|chef" }
}
