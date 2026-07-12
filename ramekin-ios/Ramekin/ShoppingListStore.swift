import CoreData
import Foundation
import Network

/// Manages shopping list operations with offline-first CoreData storage and server sync
@MainActor
class ShoppingListStore: ObservableObject {
    static let shared = ShoppingListStore()

    @Published var items: [ShoppingItem] = []
    @Published var isSyncing = false
    @Published var isOnline = true
    @Published var lastSyncError: String?
    /// Canonical category display order, served by the API and persisted
    /// across launches. Empty until the first successful sync.
    @Published var categoryOrder: [String] = []

    private let coreDataStack: CoreDataStack
    private let userDefaults: UserDefaults
    private let syncItems: (SyncRequest) async throws -> SyncResponse
    private let networkMonitor = NWPathMonitor()
    private let monitorQueue = DispatchQueue(label: "NetworkMonitor")
    private let automaticallySync: Bool
    private var activeAccountKey: String?

    private static let lastSyncAtKeyPrefix = "shopping_list_last_sync_at"
    private static let categoryOrderKeyPrefix = "shopping_list_category_order"
    private static let legacyMigrationKey = "shopping_list_account_scope_migrated"

    init(
        coreDataStack: CoreDataStack = .shared,
        userDefaults: UserDefaults = .standard,
        initialAccountKey: String? = AccountScope.currentAccountKey(),
        automaticallySync: Bool = true,
        syncItems: @escaping (SyncRequest) async throws -> SyncResponse = {
            try await ShoppingListAPI.syncItems(syncRequest: $0)
        }
    ) {
        self.coreDataStack = coreDataStack
        self.userDefaults = userDefaults
        self.automaticallySync = automaticallySync
        self.syncItems = syncItems

        if automaticallySync {
            networkMonitor.pathUpdateHandler = { [weak self] path in
                Task { @MainActor in
                    self?.isOnline = path.status == .satisfied
                    if path.status == .satisfied {
                        await self?.syncIfNeeded()
                    }
                }
            }
            networkMonitor.start(queue: monitorQueue)
        }
        setActiveAccountKey(initialAccountKey)
    }

    deinit { networkMonitor.cancel() }
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

    func addItem(
        name: String,
        amount: String? = nil,
        note: String? = nil,
        sourceRecipeId: UUID? = nil,
        sourceRecipeTitle: String? = nil
    ) {
        guard let activeAccountKey else {
            preconditionFailure("Cannot add a shopping item without an active account")
        }
        let maxSort = items.map(\.sortOrder).max() ?? -1
        _ = ShoppingItem.create(
            in: coreDataStack.viewContext, accountKey: activeAccountKey,
            item: name, amount: amount, note: note,
            sourceRecipeId: sourceRecipeId, sourceRecipeTitle: sourceRecipeTitle, sortOrder: maxSort + 1
        )
        saveAndSync()
    }

    func addItemsFromRecipe(
        ingredients: [(name: String, amount: String?)],
        recipeId: UUID,
        recipeTitle: String
    ) throws {
        guard let activeAccountKey else {
            preconditionFailure("Cannot add recipe items without an active account")
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
        validateActiveAccount(for: item)
        item.isChecked.toggle()
        item.markUpdated()
        saveAndSync()
    }

    func updateItem(_ item: ShoppingItem, name: String? = nil, amount: String? = nil, note: String? = nil) {
        validateActiveAccount(for: item)
        if let name = name { item.item = name }
        if let amount = amount { item.amount = amount }
        if let note = note { item.note = note }
        item.markUpdated()
        saveAndSync()
    }

    func updateCategoryOverride(_ item: ShoppingItem, categoryOverride: String?) {
        validateActiveAccount(for: item)
        guard item.categoryOverride != categoryOverride else { return }
        ShoppingListMutationSupport.updateCategoryOverride(item, categoryOverride: categoryOverride)
        saveAndSync()
    }

    func deleteItem(_ item: ShoppingItem) {
        validateActiveAccount(for: item)
        if item.syncStatusEnum == .pendingCreate {
            coreDataStack.viewContext.delete(item)
        } else {
            item.markDeleted()
        }
        saveAndSync()
    }

    func clearChecked() {
        for item in items where item.isChecked {
            if item.syncStatusEnum == .pendingCreate {
                coreDataStack.viewContext.delete(item)
            } else {
                item.markDeleted()
            }
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
            let creates = pending.filter { $0.syncStatusEnum == .pendingCreate }.count
            let updates = pending.filter { $0.syncStatusEnum == .pendingUpdate }.count
            let deletes = pending.filter { $0.syncStatusEnum == .pendingDelete }.count
            logger.log(
                "syncWithServer: \(pending.count) pending (\(creates) create, \(updates) update, \(deletes) delete)",
                source: "Shopping"
            )
            let request = buildSyncRequest(from: pending, accountKey: syncAccountKey)
            let response = try await logger.timed("shopping sync API", source: "Shopping") {
                try await syncItems(request)
            }
            processServerResponse(
                response,
                pendingItems: pending,
                syncStartedAt: syncStartedAt,
                accountKey: syncAccountKey
            )
            setLastSyncAt(response.syncTimestamp, accountKey: syncAccountKey)
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

        // If the sync succeeded and new items were modified during it, do one follow-up sync.
        // Only re-sync once to avoid unbounded loops from persistent conflicts.
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

    private func buildSyncRequest(from pending: [ShoppingItem], accountKey: String) -> SyncRequest {
        var creates: [SyncCreateItem] = []
        var updates: [SyncUpdateItem] = []
        var deletes: [UUID] = []

        for item in pending {
            guard let itemId = item.id else { continue }
            switch item.syncStatusEnum {
            case .pendingCreate:
                creates.append(SyncCreateItem(
                    amount: item.amount, categoryOverride: item.categoryOverride,
                    clientId: itemId, isChecked: item.isChecked,
                    item: item.item ?? "", note: item.note, sortOrder: Int(item.sortOrder),
                    sourceRecipeId: item.sourceRecipeId, sourceRecipeTitle: item.sourceRecipeTitle
                ))
            case .pendingUpdate:
                updates.append(SyncUpdateItem(
                    amount: item.amount, categoryOverride: item.categoryOverride,
                    clearCategoryOverride: item.clearCategoryOverride ? true : nil,
                    expectedVersion: Int(item.serverVersion), id: itemId,
                    isChecked: item.isChecked, item: item.item, note: item.note, sortOrder: Int(item.sortOrder)
                ))
            case .pendingDelete:
                deletes.append(itemId)
            case .synced:
                break
            }
        }

        return SyncRequest(
            creates: creates.isEmpty ? nil : creates,
            deletes: deletes.isEmpty ? nil : deletes,
            lastSyncAt: lastSyncAt(accountKey: accountKey),
            updates: updates.isEmpty ? nil : updates
        )
    }

    private func processServerResponse(
        _ response: SyncResponse,
        pendingItems: [ShoppingItem],
        syncStartedAt: Date,
        accountKey: String
    ) {
        let context = coreDataStack.viewContext

        for created in response.created {
            if let local = pendingItems.first(where: { $0.id == created.clientId }) {
                local.id = created.serverId
                ShoppingListSyncSupport.reconcileSyncedItem(
                    local,
                    version: created.version,
                    success: true,
                    syncStartedAt: syncStartedAt
                )
            }
        }

        for updated in response.updated {
            if let local = pendingItems.first(where: { $0.id == updated.id }) {
                ShoppingListSyncSupport.reconcileSyncedItem(
                    local,
                    version: updated.version,
                    success: updated.success,
                    syncStartedAt: syncStartedAt
                )
            }
        }

        for deletedId in response.deleted {
            if let local = (try? context.fetch(
                ShoppingItem.fetchById(deletedId, accountKey: accountKey)
            ))?.first {
                context.delete(local)
            }
        }

        for change in response.serverChanges {
            applyServerChange(change, accountKey: accountKey, in: context)
        }

        coreDataStack.saveContext()
    }

    private func applyServerChange(
        _ change: SyncServerChange,
        accountKey: String,
        in context: NSManagedObjectContext
    ) {
        let existing = (try? context.fetch(
            ShoppingItem.fetchById(change.id, accountKey: accountKey)
        ))?.first

        if let item = existing {
            // Don't overwrite pending local changes — they'll be synced next round
            guard item.syncStatusEnum == .synced else { return }
            guard change.version >= item.serverVersion else { return }
            item.item = change.item
            item.amount = change.amount
            item.note = change.note
            item.isChecked = change.isChecked
            item.sortOrder = Int32(change.sortOrder)
            item.sourceRecipeId = change.sourceRecipeId
            item.sourceRecipeTitle = change.sourceRecipeTitle
            item.categoryOverride = change.categoryOverride
            item.computedCategory = change.computedCategory
            item.category = change.category
            item.updatedAt = change.updatedAt
            item.markSynced(serverVersion: Int32(change.version))
        } else {
            let newItem = ShoppingItem(context: context)
            newItem.accountKey = accountKey
            newItem.id = change.id
            newItem.item = change.item
            newItem.amount = change.amount
            newItem.note = change.note
            newItem.isChecked = change.isChecked
            newItem.sortOrder = Int32(change.sortOrder)
            newItem.sourceRecipeId = change.sourceRecipeId
            newItem.sourceRecipeTitle = change.sourceRecipeTitle
            newItem.categoryOverride = change.categoryOverride
            newItem.computedCategory = change.computedCategory
            newItem.category = change.category
            newItem.createdAt = Date()
            newItem.updatedAt = change.updatedAt
            newItem.markSynced(serverVersion: Int32(change.version))
        }
    }

    private func validateActiveAccount(for item: ShoppingItem) {
        precondition(
            item.accountKey == activeAccountKey && activeAccountKey != nil,
            "Shopping item does not belong to the active account"
        )
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
            fatalError("Failed to migrate unscoped shopping items: \(error)")
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
