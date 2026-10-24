import CoreData
import XCTest
@testable import Ramekin

// MARK: - Undo and history

extension ShoppingListStoreTests {
    func testUndoingASyncedClearRestoresTheItemsAsTheyWere() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [SyncShoppingListRequest] = []
        var responses: [SyncShoppingListResponse] = []
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: {
                requests.append($0)
                return responses.removeFirst()
            }
        )
        store.addItem(name: "Milk")
        let id = try XCTUnwrap(store.items.first?.id)
        responses.append(Self.response(
            results: [.applied], changes: [Self.serverChange(id: id, item: "Milk", isChecked: true)]
        ))
        await store.syncWithServer(isFollowUp: true)

        let undo = try XCTUnwrap(store.clearChecked())
        XCTAssertEqual(undo.message, "Cleared 1 item")
        responses.append(Self.response(results: [.applied], deleted: [id]))
        await store.syncWithServer(isFollowUp: true)
        XCTAssertTrue(store.items.isEmpty)

        store.restore(undo)
        XCTAssertEqual(store.items.map(\.item), ["Milk"])
        XCTAssertEqual(store.items.first?.isChecked, true)
        responses.append(Self.response(
            results: [.applied], changes: [Self.serverChange(id: id, item: "Milk", isChecked: true, changeSeq: 3)]
        ))
        await store.syncWithServer(isFollowUp: true)

        let restore = try XCTUnwrap(requests.last?.ops?.first)
        XCTAssertEqual(restore.op, .restore)
        XCTAssertEqual(restore.id, id)
        XCTAssertEqual(restore.fields?.item, "Milk")
        XCTAssertEqual(restore.fields?.isChecked, true)
        // Every field is sent, and "" clears an empty one.
        XCTAssertEqual(restore.fields?.amount, "")
        XCTAssertEqual(store.items.first?.syncStatusEnum, .synced)
        XCTAssertEqual(store.items.first?.changeSeq, 3)
    }

    func testUndoingADeleteInFlightSendsTheRestoreNext() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [SyncShoppingListRequest] = []
        var responses: [SyncShoppingListResponse] = []
        var duringSync: (() -> Void)?
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: { request in
                requests.append(request)
                duringSync?()
                duringSync = nil
                return responses.removeFirst()
            }
        )
        store.addItem(name: "Eggs")
        let id = try XCTUnwrap(store.items.first?.id)
        responses.append(Self.response(results: [.applied], changes: [Self.serverChange(id: id, item: "Eggs")]))
        await store.syncWithServer(isFollowUp: true)

        let undo = try XCTUnwrap(store.deleteItem(try XCTUnwrap(store.items.first)))
        XCTAssertEqual(undo.message, "Deleted Eggs")
        duringSync = { store.restore(undo) }
        // The delete's own tombstone doesn't drop the restored row.
        responses.append(Self.response(results: [.applied], deleted: [id]))
        await store.syncWithServer(isFollowUp: true)

        XCTAssertEqual(store.items.map(\.item), ["Eggs"])
        XCTAssertEqual(store.items.first?.syncStatusEnum, .pendingRestore)
        responses.append(Self.response(
            results: [.applied], changes: [Self.serverChange(id: id, item: "Eggs", changeSeq: 3)]
        ))
        await store.syncWithServer(isFollowUp: true)
        XCTAssertEqual(requests.last?.ops?.map(\.op), [.restore])
        XCTAssertEqual(store.items.first?.syncStatusEnum, .synced)
    }

    func testUndoingADeleteThatComesBackGoneStillSendsTheRestore() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [SyncShoppingListRequest] = []
        var responses: [SyncShoppingListResponse] = []
        var duringSync: (() -> Void)?
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: { request in
                requests.append(request)
                duringSync?()
                duringSync = nil
                return responses.removeFirst()
            }
        )
        // Added and deleted offline: the server never saw the create, so
        // the delete is gone. Undo was tapped while it was in flight.
        store.addItem(name: "Jam")
        let id = try XCTUnwrap(store.items.first?.id)
        let undo = try XCTUnwrap(store.deleteItem(try XCTUnwrap(store.items.first)))
        duringSync = { store.restore(undo) }
        responses.append(Self.response(results: [.gone], deleted: [id]))
        await store.syncWithServer(isFollowUp: true)

        XCTAssertEqual(store.items.map(\.item), ["Jam"])
        responses.append(Self.response(
            results: [.applied], changes: [Self.serverChange(id: id, item: "Jam", changeSeq: 2)]
        ))
        await store.syncWithServer(isFollowUp: true)
        let restore = try XCTUnwrap(requests.last?.ops?.first)
        XCTAssertEqual(restore.op, .restore)
        XCTAssertEqual(restore.fields?.item, "Jam")
        XCTAssertEqual(store.items.first?.syncStatusEnum, .synced)
    }

    func testUndoingAnUnsentDeleteSendsOnlyTheRestore() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [SyncShoppingListRequest] = []
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: {
                requests.append($0)
                return Self.response(results: [.applied])
            }
        )
        // Added offline, so its create may never reach the server: the
        // restore carries the item to create it from.
        store.addItem(name: "Bread")
        let undo = try XCTUnwrap(store.deleteItem(try XCTUnwrap(store.items.first)))
        store.restore(undo)

        await store.syncWithServer(isFollowUp: true)

        let restore = try XCTUnwrap(requests.first?.ops)
        XCTAssertEqual(restore.map(\.op), [.restore])
        XCTAssertEqual(restore.first?.fields?.item, "Bread")
        XCTAssertEqual(store.items.first?.syncStatusEnum, .synced)
    }

    func testUndoingAHistoryEntryGoesThroughTheServerThenSyncs() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var undone: [UUID] = []
        var syncs = 0
        let entryId = UUID()
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: { _ in
                syncs += 1
                return Self.response()
            },
            undoHistoryEntry: {
                undone.append($0)
                return ShoppingListHistoryEntry(createdAt: Date(), id: UUID(), summary: "Restored milk", undoesId: $0)
            }
        )

        try await store.undo(entry: entryId)

        XCTAssertEqual(undone, [entryId])
        XCTAssertEqual(syncs, 1)
    }
}
