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

    private let coreDataStack = CoreDataStack.shared
    private let networkMonitor = NWPathMonitor()
    private let monitorQueue = DispatchQueue(label: "NetworkMonitor")
    private let lastSyncAtKey = "shopping_list_last_sync_at"

    private var lastSyncAt: Date? {
        get { UserDefaults.standard.object(forKey: lastSyncAtKey) as? Date }
        set { UserDefaults.standard.set(newValue, forKey: lastSyncAtKey) }
    }

    private init() {
        networkMonitor.pathUpdateHandler = { [weak self] path in
            Task { @MainActor in
                self?.isOnline = path.status == .satisfied
                if path.status == .satisfied {
                    await self?.syncIfNeeded()
                }
            }
        }
        networkMonitor.start(queue: monitorQueue)
        fetchItems()
    }

    deinit { networkMonitor.cancel() }

    // MARK: - Local Operations

    func fetchItems() {
        let request = ShoppingItem.fetchActiveItems()
        items = (try? coreDataStack.viewContext.fetch(request)) ?? []
    }

    func addItem(
        name: String,
        amount: String? = nil,
        note: String? = nil,
        sourceRecipeId: UUID? = nil,
        sourceRecipeTitle: String? = nil
    ) {
        let maxSort = items.map(\.sortOrder).max() ?? -1
        _ = ShoppingItem.create(
            in: coreDataStack.viewContext, item: name, amount: amount, note: note,
            sourceRecipeId: sourceRecipeId, sourceRecipeTitle: sourceRecipeTitle, sortOrder: maxSort + 1
        )
        saveAndSync()
    }

    func addItemsFromRecipe(ingredients: [(name: String, amount: String?)], recipeId: UUID, recipeTitle: String) {
        var maxSort = items.map(\.sortOrder).max() ?? -1
        for ingredient in ingredients {
            maxSort += 1
            _ = ShoppingItem.create(
                in: coreDataStack.viewContext, item: ingredient.name, amount: ingredient.amount,
                sourceRecipeId: recipeId, sourceRecipeTitle: recipeTitle, sortOrder: maxSort
            )
        }
        saveAndSync()
    }

    func toggleChecked(_ item: ShoppingItem) {
        item.isChecked.toggle()
        item.markUpdated()
        saveAndSync()
    }

    func updateItem(_ item: ShoppingItem, name: String? = nil, amount: String? = nil, note: String? = nil) {
        if let name = name { item.item = name }
        if let amount = amount { item.amount = amount }
        if let note = note { item.note = note }
        item.markUpdated()
        saveAndSync()
    }

    func deleteItem(_ item: ShoppingItem) {
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
        guard isOnline else { return }
        Task { await syncWithServer() }
    }

    func syncIfNeeded() async {
        guard isOnline, !isSyncing else { return }
        let hasPending = (try? coreDataStack.viewContext.fetch(ShoppingItem.fetchPendingSync()))?.isEmpty == false
        let stale = lastSyncAt == nil || Date().timeIntervalSince(lastSyncAt!) > 300
        if hasPending || stale { await syncWithServer() }
    }

    func syncWithServer() async {
        let logger = DebugLogger.shared
        guard isOnline, !isSyncing else {
            logger.log("syncWithServer skipped (online=\(isOnline), syncing=\(isSyncing))", source: "Shopping")
            return
        }
        isSyncing = true
        lastSyncError = nil
        logger.log("syncWithServer started", source: "Shopping")

        do {
            let pending = try coreDataStack.viewContext.fetch(ShoppingItem.fetchPendingSync())
            logger.log("syncWithServer: \(pending.count) pending items", source: "Shopping")
            let request = buildSyncRequest(from: pending)
            let response = try await logger.timed("shopping sync API", source: "Shopping") {
                try await ShoppingListAPI.syncItems(syncRequest: request)
            }
            processServerResponse(response, pendingItems: pending)
            lastSyncAt = response.syncTimestamp
            logger.log("syncWithServer completed successfully", source: "Shopping")
        } catch {
            logger.log("syncWithServer FAILED: \(error.localizedDescription)", source: "Shopping")
            lastSyncError = error.localizedDescription
        }

        isSyncing = false
        fetchItems()
    }

    private func buildSyncRequest(from pending: [ShoppingItem]) -> SyncRequest {
        var creates: [SyncCreateItem] = []
        var updates: [SyncUpdateItem] = []
        var deletes: [UUID] = []

        for item in pending {
            guard let itemId = item.id else { continue }
            switch item.syncStatusEnum {
            case .pendingCreate:
                creates.append(SyncCreateItem(
                    amount: item.amount, clientId: itemId, isChecked: item.isChecked,
                    item: item.item ?? "", note: item.note, sortOrder: Int(item.sortOrder),
                    sourceRecipeId: item.sourceRecipeId, sourceRecipeTitle: item.sourceRecipeTitle
                ))
            case .pendingUpdate:
                updates.append(SyncUpdateItem(
                    amount: item.amount, expectedVersion: Int(item.serverVersion), id: itemId,
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
            lastSyncAt: lastSyncAt,
            updates: updates.isEmpty ? nil : updates
        )
    }

    private func processServerResponse(_ response: SyncResponse, pendingItems: [ShoppingItem]) {
        let context = coreDataStack.viewContext

        for created in response.created {
            if let local = pendingItems.first(where: { $0.id == created.clientId }) {
                local.id = created.serverId
                local.markSynced(serverVersion: Int32(created.version))
            }
        }

        for updated in response.updated where updated.success {
            if let local = pendingItems.first(where: { $0.id == updated.id }) {
                local.markSynced(serverVersion: Int32(updated.version))
            }
        }

        for deletedId in response.deleted {
            if let local = (try? context.fetch(ShoppingItem.fetchById(deletedId)))?.first {
                context.delete(local)
            }
        }

        for change in response.serverChanges {
            applyServerChange(change, in: context)
        }

        coreDataStack.saveContext()
    }

    private func applyServerChange(_ change: SyncServerChange, in context: NSManagedObjectContext) {
        let existing = (try? context.fetch(ShoppingItem.fetchById(change.id)))?.first

        if let item = existing {
            guard change.version >= item.serverVersion else { return }
            item.item = change.item
            item.amount = change.amount
            item.note = change.note
            item.isChecked = change.isChecked
            item.sortOrder = Int32(change.sortOrder)
            item.sourceRecipeId = change.sourceRecipeId
            item.sourceRecipeTitle = change.sourceRecipeTitle
            item.category = change.category
            item.updatedAt = change.updatedAt
            item.markSynced(serverVersion: Int32(change.version))
        } else {
            let newItem = ShoppingItem(context: context)
            newItem.id = change.id
            newItem.item = change.item
            newItem.amount = change.amount
            newItem.note = change.note
            newItem.isChecked = change.isChecked
            newItem.sortOrder = Int32(change.sortOrder)
            newItem.sourceRecipeId = change.sourceRecipeId
            newItem.sourceRecipeTitle = change.sourceRecipeTitle
            newItem.category = change.category
            newItem.createdAt = Date()
            newItem.updatedAt = change.updatedAt
            newItem.markSynced(serverVersion: Int32(change.version))
        }
    }

}
