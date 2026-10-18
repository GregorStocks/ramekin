import CoreData
import Foundation

/// Manages shopping list operations with offline-first CoreData storage and server sync
@MainActor
class ShoppingListStore: ObservableObject {
    enum ShoppingListError: LocalizedError {
        case noActiveAccount

        var errorDescription: String? {
            switch self {
            case .noActiveAccount:
                return "Sign in to add items to your shopping list."
            }
        }
    }

    static let shared = ShoppingListStore()

    @Published var items: [ShoppingItem] = []
    @Published var isSyncing = false
    @Published var isOnline = true
    @Published var lastSyncError: String?
    /// Canonical category display order, served by the API and persisted
    /// across launches. Empty until the first successful sync.
    @Published var categoryOrder: [String] = []

    private let coreDataStack: CoreDataStack
    /// Unsynced edits are lost on quit; the UI warns about it.
    var isUsingVolatileStorage: Bool { coreDataStack.isUsingVolatileStore }
    private let userDefaults: UserDefaults
    private let syncItems: (SyncShoppingListRequest) async throws -> SyncShoppingListResponse
    private let automaticallySync: Bool
    private var activeAccountKey: String?

    /// Wall-clock time of the last sync, used only to decide when a sync is stale.
    private static let lastSyncAtKeyPrefix = "shopping_list_last_sync_at"
    /// Server change cursor to resume incremental sync from. Versioned with
    /// the sync protocol (doc/sync-design.md): a new one starts with a full
    /// resync.
    private static let syncCursor = SyncCursorStore(prefix: "shopping_list_sync_cursor_v2")
    private static let categoryOrderKeyPrefix = "shopping_list_category_order"
    private static let legacyMigrationKey = "shopping_list_account_scope_migrated"

    init(
        coreDataStack: CoreDataStack = .shared,
        userDefaults: UserDefaults = .standard,
        initialAccountKey: String? = AccountScope.currentAccountKey(),
        automaticallySync: Bool = true,
        networkMonitor: NetworkMonitor = .shared,
        syncItems: @escaping (SyncShoppingListRequest) async throws -> SyncShoppingListResponse = {
            try await SyncAPI.syncShoppingList(syncShoppingListRequest: $0)
        }
    ) {
        self.coreDataStack = coreDataStack
        self.userDefaults = coreDataStack.syncStateDefaults(userDefaults)
        self.automaticallySync = automaticallySync
        self.syncItems = syncItems

        if automaticallySync {
            isOnline = networkMonitor.isOnline
            networkMonitor.observe { [weak self] online in
                self?.isOnline = online
                if online {
                    Task { await self?.syncIfNeeded() }
                }
            }
        }
        setActiveAccountKey(initialAccountKey)
    }
}

extension ShoppingListStore {
    // MARK: - Local Operations

    func setActiveAccountKey(_ accountKey: String?) {
        migrateLegacyState(activeAccountKey: accountKey)
        activeAccountKey = accountKey
        items = []
        categoryOrder = []
        lastSyncError = nil

        guard let accountKey else { return }
        categoryOrder = userDefaults.stringArray(forKey: categoryOrderKey(accountKey: accountKey)) ?? []
        fetchItems()
        if automaticallySync, isOnline {
            Task { await syncIfNeeded() }
        }
    }

    func fetchItems() {
        guard let activeAccountKey else {
            items = []
            return
        }
        let request = ShoppingItem.fetchActiveItems(accountKey: activeAccountKey)
        items = (try? coreDataStack.viewContext.fetch(request)) ?? []
    }

    /// False when there is no active account to add to (e.g. mid-logout).
    @discardableResult
    func addItem(
        name: String,
        amount: String? = nil,
        note: String? = nil,
        sourceRecipeId: UUID? = nil,
        sourceRecipeTitle: String? = nil
    ) -> Bool {
        guard let activeAccountKey else {
            DebugLogger.shared.log("Ignoring shopping item add with no active account", source: "ShoppingList")
            return false
        }
        let maxSort = items.map(\.sortOrder).max() ?? -1
        _ = ShoppingItem.create(
            in: coreDataStack.viewContext, accountKey: activeAccountKey,
            item: name, amount: amount, note: note,
            sourceRecipeId: sourceRecipeId, sourceRecipeTitle: sourceRecipeTitle, sortOrder: maxSort + 1
        )
        saveAndSync()
        return true
    }

    func addItemsFromRecipe(
        ingredients: [(name: String, amount: String?)],
        recipeId: UUID,
        recipeTitle: String
    ) throws {
        guard let activeAccountKey else {
            throw ShoppingListError.noActiveAccount
        }
        try ShoppingListMutationSupport.addItemsFromRecipe(
            ingredients: ingredients,
            recipe: (id: recipeId, title: recipeTitle),
            accountKey: activeAccountKey,
            context: coreDataStack.viewContext,
            save: coreDataStack.saveContextOrThrow
        )
        fetchItems()
        triggerSync()
    }

    func toggleChecked(_ item: ShoppingItem) {
        guard isActiveAccountItem(item) else { return }
        item.isChecked.toggle()
        item.markUpdated(["is_checked"])
        saveAndSync()
    }

    func updateItem(_ item: ShoppingItem, name: String? = nil, amount: String? = nil, note: String? = nil) {
        guard isActiveAccountItem(item) else { return }
        var fields: Set<String> = []
        if let name = name {
            item.item = name
            fields.insert("item")
        }
        if let amount = amount {
            item.amount = amount
            fields.insert("amount")
        }
        if let note = note {
            item.note = note
            fields.insert("note")
        }
        guard !fields.isEmpty else { return }
        item.markUpdated(fields)
        saveAndSync()
    }

    func updateCategoryOverride(_ item: ShoppingItem, categoryOverride: String?) {
        guard isActiveAccountItem(item) else { return }
        guard item.categoryOverride != categoryOverride else { return }
        ShoppingListMutationSupport.updateCategoryOverride(item, categoryOverride: categoryOverride)
        saveAndSync()
    }

    /// Even an item whose create looks unsent is deleted through the server:
    /// the create may have reached it on a sync whose response was lost.
    func deleteItem(_ item: ShoppingItem) {
        guard isActiveAccountItem(item) else { return }
        item.markDeleted()
        saveAndSync()
    }

    func clearChecked() {
        for item in items where item.isChecked {
            item.markDeleted()
        }
        saveAndSync()
    }

    private func saveAndSync() {
        coreDataStack.saveContext()
        fetchItems()
        triggerSync()
    }

    // MARK: - Sync

    private func triggerSync() {
        guard automaticallySync, isOnline else { return }
        Task { await syncWithServer() }
    }

    func syncIfNeeded() async {
        guard let activeAccountKey, isOnline, !isSyncing else { return }
        let hasPending = (try? coreDataStack.viewContext.fetch(
            ShoppingItem.fetchPendingSync(accountKey: activeAccountKey)
        ))?.isEmpty == false
        let lastSyncAt = lastSyncAt(accountKey: activeAccountKey)
        let stale = lastSyncAt == nil || Date().timeIntervalSince(lastSyncAt!) > 300
        if hasPending || stale { await syncWithServer() }
    }

    func syncWithServer(isFollowUp: Bool = false) async {
        let logger = DebugLogger.shared
        guard let syncAccountKey = activeAccountKey, isOnline, !isSyncing else {
            logger.log("syncWithServer skipped (online=\(isOnline), syncing=\(isSyncing))", source: "Shopping")
            return
        }
        isSyncing = true
        lastSyncError = nil
        let syncStartedAt = Date()
        logger.log("syncWithServer started", source: "Shopping")

        do {
            let pending = try coreDataStack.viewContext.fetch(
                ShoppingItem.fetchPendingSync(accountKey: syncAccountKey)
            )
            let outbox = ShoppingListSyncSupport.outbox(for: pending)
            logger.log("syncWithServer: \(outbox.ops.count) pending ops", source: "Shopping")
            let request = SyncShoppingListRequest(
                cursor: Self.syncCursor.cursor(accountKey: syncAccountKey, in: userDefaults),
                ops: outbox.ops.isEmpty ? nil : outbox.ops
            )
            let response = try await logger.timed("shopping sync API", source: "Shopping") {
                try await syncItems(request)
            }
            processServerResponse(response, outbox: outbox, accountKey: syncAccountKey)
            setLastSyncAt(syncStartedAt, accountKey: syncAccountKey)
            Self.syncCursor.set(response.cursor, accountKey: syncAccountKey, in: userDefaults)
            userDefaults.set(response.categoryOrder, forKey: categoryOrderKey(accountKey: syncAccountKey))
            if activeAccountKey == syncAccountKey {
                categoryOrder = response.categoryOrder
            }
            logger.log("syncWithServer completed successfully", source: "Shopping")
        } catch {
            logger.log("syncWithServer FAILED: \(error.localizedDescription)", source: "Shopping")
            if activeAccountKey == syncAccountKey {
                lastSyncError = error.localizedDescription
            }
        }

        isSyncing = false
        fetchItems()

        guard activeAccountKey == syncAccountKey else {
            if automaticallySync {
                await syncIfNeeded()
            }
            return
        }

        // Edits made while the sync was in flight are still pending: send
        // them in one follow-up sync, not a loop.
        if lastSyncError == nil && !isFollowUp {
            let hasPending = (try? coreDataStack.viewContext.fetch(
                ShoppingItem.fetchPendingSync(accountKey: syncAccountKey)
            ))?.isEmpty == false
            if hasPending && isOnline {
                logger.log("syncWithServer: still have pending items, follow-up sync", source: "Shopping")
                await syncWithServer(isFollowUp: true)
            }
        }
    }

    /// Settle each op from its result, then apply the server's changes,
    /// whose own-write echoes now overwrite only fields edited mid-sync.
    private func processServerResponse(
        _ response: SyncShoppingListResponse,
        outbox: ShoppingListSyncSupport.Outbox,
        accountKey: String
    ) {
        let context = coreDataStack.viewContext

        for (operation, result) in zip(outbox.ops, response.results) {
            guard let sent = outbox.sent[operation.id],
                  let local = (try? context.fetch(
                      ShoppingItem.fetchById(operation.id, accountKey: accountKey)
                  ))?.first else { continue }
            if local.settle(result: result, sent: sent) == .removed {
                context.delete(local)
            }
        }

        ShoppingListSyncSupport.apply(response, accountKey: accountKey, in: context)

        coreDataStack.saveContext()
    }

    /// False for a stale item from another account (e.g. the account
    /// switched while a row was on screen); callers skip the mutation.
    private func isActiveAccountItem(_ item: ShoppingItem) -> Bool {
        guard item.accountKey == activeAccountKey, activeAccountKey != nil else {
            DebugLogger.shared.log("Ignoring mutation of a shopping item outside the active account", source: "ShoppingList")
            return false
        }
        return true
    }

    private func lastSyncAt(accountKey: String) -> Date? {
        userDefaults.object(forKey: lastSyncAtKey(accountKey: accountKey)) as? Date
    }

    private func setLastSyncAt(_ date: Date, accountKey: String) {
        userDefaults.set(date, forKey: lastSyncAtKey(accountKey: accountKey))
    }

    private func lastSyncAtKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(prefix: Self.lastSyncAtKeyPrefix, accountKey: accountKey)
    }

    private func categoryOrderKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(prefix: Self.categoryOrderKeyPrefix, accountKey: accountKey)
    }

    private func migrateLegacyState(activeAccountKey: String?) {
        guard let activeAccountKey,
              !userDefaults.bool(forKey: Self.legacyMigrationKey) else {
            return
        }

        do {
            let unscopedItems = try coreDataStack.viewContext.fetch(ShoppingItem.fetchUnscopedItems())
            for item in unscopedItems {
                item.accountKey = activeAccountKey
            }
            try coreDataStack.saveContextOrThrow()
        } catch {
            // Leave the migration flag unset so the next launch retries.
            coreDataStack.viewContext.rollback()
            DebugLogger.shared.log("Failed to migrate unscoped shopping items: \(error)", source: "ShoppingList")
            return
        }

        if let legacyLastSyncAt = userDefaults.object(forKey: Self.lastSyncAtKeyPrefix) as? Date {
            setLastSyncAt(legacyLastSyncAt, accountKey: activeAccountKey)
        }
        if let legacyCategoryOrder = userDefaults.stringArray(forKey: Self.categoryOrderKeyPrefix) {
            userDefaults.set(
                legacyCategoryOrder,
                forKey: categoryOrderKey(accountKey: activeAccountKey)
            )
        }
        userDefaults.removeObject(forKey: Self.lastSyncAtKeyPrefix)
        userDefaults.removeObject(forKey: Self.categoryOrderKeyPrefix)
        userDefaults.set(true, forKey: Self.legacyMigrationKey)
    }

}
