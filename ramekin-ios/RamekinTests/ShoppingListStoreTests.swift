import CoreData
import XCTest
@testable import Ramekin

@MainActor
final class ShoppingListStoreTests: XCTestCase {
    func testItemsAndPendingSyncAreIsolatedByUserAndServer() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [SyncRequest] = []
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: { request in
                requests.append(request)
                return Self.emptyResponse()
            }
        )

        store.addItem(name: "Apples")
        store.setActiveAccountKey(secondUserAccount)
        XCTAssertTrue(store.items.isEmpty)
        store.addItem(name: "Bananas")
        await store.syncWithServer(isFollowUp: true)

        store.setActiveAccountKey(secondServerAccount)
        XCTAssertTrue(store.items.isEmpty)
        store.addItem(name: "Carrots")
        await store.syncWithServer(isFollowUp: true)

        XCTAssertEqual(requests.count, 2)
        XCTAssertEqual(requests[0].creates?.map(\.item), ["Bananas"])
        XCTAssertEqual(requests[1].creates?.map(\.item), ["Carrots"])
        XCTAssertEqual(
            try stack.viewContext.fetch(ShoppingItem.fetchActiveItems(accountKey: firstAccount)).map(\.item),
            ["Apples"]
        )
        XCTAssertEqual(
            try stack.viewContext.fetch(ShoppingItem.fetchActiveItems(accountKey: secondUserAccount)).map(\.item),
            ["Bananas"]
        )
        XCTAssertEqual(
            try stack.viewContext.fetch(ShoppingItem.fetchActiveItems(accountKey: secondServerAccount)).map(\.item),
            ["Carrots"]
        )
    }

    func testAccountChangeAndLogoutReplacePublishedState() {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false
        )

        store.addItem(name: "Apples")
        XCTAssertEqual(store.items.map(\.item), ["Apples"])

        store.setActiveAccountKey(secondUserAccount)
        XCTAssertTrue(store.items.isEmpty)
        store.addItem(name: "Bananas")

        store.setActiveAccountKey(nil)
        XCTAssertTrue(store.items.isEmpty)
        XCTAssertTrue(store.categoryOrder.isEmpty)

        store.setActiveAccountKey(firstAccount)
        XCTAssertEqual(store.items.map(\.item), ["Apples"])
    }

    func testLegacyRowsAndPreferencesMigrateToActiveAccount() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let legacyItem = ShoppingItem.create(
            in: stack.viewContext,
            accountKey: "temporary",
            item: "Apples"
        )
        legacyItem.accountKey = nil
        try stack.viewContext.save()
        let legacySyncDate = Date(timeIntervalSince1970: 500)
        defaults.set(legacySyncDate, forKey: "shopping_list_last_sync_at")
        defaults.set(["Produce", "Other"], forKey: "shopping_list_category_order")
        var request: SyncRequest?

        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: nil,
            automaticallySync: false,
            syncItems: {
                request = $0
                return Self.emptyResponse()
            }
        )

        XCTAssertTrue(store.items.isEmpty)
        XCTAssertNil(legacyItem.accountKey)
        XCTAssertEqual(defaults.object(forKey: "shopping_list_last_sync_at") as? Date, legacySyncDate)

        store.setActiveAccountKey(firstAccount)

        XCTAssertEqual(store.items.map(\.item), ["Apples"])
        XCTAssertEqual(store.items.first?.accountKey, firstAccount)
        XCTAssertEqual(store.categoryOrder, ["Produce", "Other"])
        await store.syncWithServer(isFollowUp: true)
        XCTAssertEqual(request?.lastSyncAt, legacySyncDate)
        XCTAssertNil(defaults.object(forKey: "shopping_list_last_sync_at"))
        XCTAssertNil(defaults.object(forKey: "shopping_list_category_order"))
    }

    func testInFlightResponseStaysWithOriginatingAccount() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let syncStub = ShoppingSyncStub()
        let store = ShoppingListStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: firstAccount,
            automaticallySync: false,
            syncItems: { try await syncStub.sync($0) }
        )
        store.addItem(name: "Apples")
        let clientId = try XCTUnwrap(store.items.first?.id)
        let serverId = UUID()

        let syncTask = Task { await store.syncWithServer() }
        await syncStub.waitForRequest()
        store.setActiveAccountKey(secondUserAccount)
        syncStub.complete(
            with: SyncResponse(
                categoryOrder: ["Produce"],
                created: [SyncCreatedItem(clientId: clientId, serverId: serverId, version: 1)],
                deleted: [],
                serverChanges: [],
                syncTimestamp: Date(timeIntervalSince1970: 800),
                updated: []
            )
        )
        await syncTask.value

        XCTAssertTrue(store.items.isEmpty)
        let firstAccountItems = try stack.viewContext.fetch(
            ShoppingItem.fetchActiveItems(accountKey: firstAccount)
        )
        XCTAssertEqual(firstAccountItems.map(\.item), ["Apples"])
        XCTAssertEqual(firstAccountItems.first?.id, serverId)
        XCTAssertEqual(firstAccountItems.first?.syncStatusEnum, .synced)
        XCTAssertTrue(
            try stack.viewContext.fetch(ShoppingItem.fetchActiveItems(accountKey: secondUserAccount)).isEmpty
        )
    }

    private static func emptyResponse() -> SyncResponse {
        SyncResponse(
            categoryOrder: [],
            created: [],
            deleted: [],
            serverChanges: [],
            syncTimestamp: Date(timeIntervalSince1970: 1_000),
            updated: []
        )
    }

    private func makeStorage() -> (CoreDataStack, UserDefaults) {
        let container = NSPersistentContainer(name: "Ramekin")
        let description = NSPersistentStoreDescription()
        description.type = NSInMemoryStoreType
        container.persistentStoreDescriptions = [description]
        container.loadPersistentStores { _, error in
            XCTAssertNil(error)
        }
        let defaults = UserDefaults(suiteName: defaultsSuiteName)!
        defaults.removePersistentDomain(forName: defaultsSuiteName)
        return (CoreDataStack(container: container), defaults)
    }

    private var defaultsSuiteName: String { "ShoppingListStoreTests.\(name)" }
    private var firstAccount: String { AccountScope.key(serverURL: "https://one.test", username: "chef") }
    private var secondUserAccount: String { AccountScope.key(serverURL: "https://one.test", username: "baker") }
    private var secondServerAccount: String { AccountScope.key(serverURL: "https://two.test", username: "chef") }
}

@MainActor
private final class ShoppingSyncStub {
    private var continuation: CheckedContinuation<SyncResponse, Error>?
    private var requestWaiters: [CheckedContinuation<Void, Never>] = []
    private(set) var requests: [SyncRequest] = []

    func sync(_ request: SyncRequest) async throws -> SyncResponse {
        requests.append(request)
        requestWaiters.forEach { $0.resume() }
        requestWaiters = []
        return try await withCheckedThrowingContinuation { continuation = $0 }
    }

    func waitForRequest() async {
        guard requests.isEmpty else { return }
        await withCheckedContinuation { requestWaiters.append($0) }
    }

    func complete(with response: SyncResponse) {
        continuation?.resume(returning: response)
        continuation = nil
    }
}
