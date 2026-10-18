import CoreData
import XCTest
@testable import Ramekin

// MARK: - Field ops

extension ShoppingListStoreTests {
    func testAnUpdateSendsOnlyTheChangedFieldAndTheEchoSettlesIt() async throws {
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
        responses.append(Self.response(results: [.applied], changes: [Self.serverChange(id: id, item: "Milk")]))
        await store.syncWithServer(isFollowUp: true)

        store.updateItem(try XCTUnwrap(store.items.first), note: "2%")
        var echo = Self.serverChange(id: id, item: "Milk", changeSeq: 2)
        echo.note = "2%"
        responses.append(Self.response(results: [.applied], changes: [echo]))
        await store.syncWithServer(isFollowUp: true)

        let update = try XCTUnwrap(requests.last?.ops?.first)
        XCTAssertEqual(update.op, .update)
        XCTAssertEqual(update.fields?.note, "2%")
        XCTAssertNil(update.fields?.item)
        XCTAssertNil(update.fields?.isChecked)
        let milk = try XCTUnwrap(store.items.first)
        XCTAssertEqual(milk.note, "2%")
        XCTAssertEqual(milk.syncStatusEnum, .synced)
    }

    func testAnEditFromAnotherDeviceAndALocalEditBothSurvive() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var responses: [SyncShoppingListResponse] = []
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: { _ in responses.removeFirst() }
        )
        store.addItem(name: "Milk")
        let id = try XCTUnwrap(store.items.first?.id)
        responses.append(Self.response(results: [.applied], changes: [Self.serverChange(id: id, item: "Milk")]))
        await store.syncWithServer(isFollowUp: true)

        // Another device checks it off while this one edits the note. The
        // server applies the note on top, and sends back both.
        store.updateItem(try XCTUnwrap(store.items.first), note: "2%")
        var merged = Self.serverChange(id: id, item: "Milk", isChecked: true, changeSeq: 4)
        merged.note = "2%"
        responses.append(Self.response(results: [.applied], changes: [merged]))
        await store.syncWithServer(isFollowUp: true)

        let milk = try XCTUnwrap(store.items.first)
        XCTAssertTrue(milk.isChecked)
        XCTAssertEqual(milk.note, "2%")
        XCTAssertEqual(milk.syncStatusEnum, .synced)
    }

    func testAnEditMadeWhileASyncIsInFlightIsSentNext() async throws {
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
        store.addItem(name: "Milk")
        let id = try XCTUnwrap(store.items.first?.id)
        responses.append(Self.response(results: [.applied], changes: [Self.serverChange(id: id, item: "Milk")]))
        await store.syncWithServer(isFollowUp: true)

        let milk = try XCTUnwrap(store.items.first)
        store.updateItem(milk, note: "2%")
        duringSync = { store.toggleChecked(milk) }
        var echo = Self.serverChange(id: id, item: "Milk", changeSeq: 2)
        echo.note = "2%"
        responses.append(Self.response(results: [.applied], changes: [echo]))
        await store.syncWithServer(isFollowUp: true)

        // The echo doesn't undo the check-off made mid-sync.
        XCTAssertTrue(milk.isChecked)
        XCTAssertEqual(milk.syncStatusEnum, .pendingUpdate)
        responses.append(Self.response(results: [.applied]))
        await store.syncWithServer(isFollowUp: true)
        let followUp = try XCTUnwrap(requests.last?.ops?.first)
        XCTAssertEqual(followUp.fields?.isChecked, true)
        XCTAssertNil(followUp.fields?.note)
    }

    func testADeleteMadeWhileAnUpdateIsInFlightIsStillSent() async throws {
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
        store.addItem(name: "Milk")
        let id = try XCTUnwrap(store.items.first?.id)
        responses.append(Self.response(results: [.applied], changes: [Self.serverChange(id: id, item: "Milk")]))
        await store.syncWithServer(isFollowUp: true)

        let milk = try XCTUnwrap(store.items.first)
        store.updateItem(milk, note: "2%")
        duringSync = { store.deleteItem(milk) }
        responses.append(Self.response(results: [.applied], changes: [Self.serverChange(id: id, item: "Milk", changeSeq: 2)]))
        await store.syncWithServer(isFollowUp: true)

        XCTAssertTrue(store.items.isEmpty)
        responses.append(Self.response(results: [.applied], deleted: [id]))
        await store.syncWithServer(isFollowUp: true)
        XCTAssertEqual(requests.last?.ops?.map(\.op), [.delete])
        XCTAssertTrue(
            try stack.viewContext.fetch(ShoppingItem.fetchPendingSync(accountKey: firstAccount)).isEmpty
        )
    }

    func testAnUpdateOfAnItemDeletedOnTheServerDropsIt() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var responses: [SyncShoppingListResponse] = []
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: { _ in responses.removeFirst() }
        )
        store.addItem(name: "Eggs")
        let id = try XCTUnwrap(store.items.first?.id)
        responses.append(Self.response(results: [.applied], changes: [Self.serverChange(id: id, item: "Eggs")]))
        await store.syncWithServer(isFollowUp: true)

        store.updateItem(try XCTUnwrap(store.items.first), note: "a dozen")
        responses.append(Self.response(results: [.gone]))
        await store.syncWithServer(isFollowUp: true)

        XCTAssertTrue(store.items.isEmpty)
        XCTAssertTrue(
            try stack.viewContext.fetch(ShoppingItem.fetchPendingSync(accountKey: firstAccount)).isEmpty
        )
    }

    func testDeletingAnUnsentItemGoesThroughTheServer() async throws {
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
                return Self.response(results: [.gone])
            }
        )
        // Its create may have reached the server on a sync whose response
        // was lost, so the delete is sent rather than dropped locally.
        store.addItem(name: "Eggs")
        store.deleteItem(try XCTUnwrap(store.items.first))
        XCTAssertTrue(store.items.isEmpty)

        await store.syncWithServer(isFollowUp: true)

        XCTAssertEqual(requests.first?.ops?.map(\.op), [.delete])
        XCTAssertTrue(
            try stack.viewContext.fetch(ShoppingItem.fetchPendingSync(accountKey: firstAccount)).isEmpty
        )
    }
}
