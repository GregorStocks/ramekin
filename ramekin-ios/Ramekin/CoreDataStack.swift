import CoreData
import Foundation

/// Manages the Core Data stack for offline shopping list storage
class CoreDataStack: ObservableObject {
    static let shared = CoreDataStack()

    let container: NSPersistentContainer

    /// The main view context for UI operations
    var viewContext: NSManagedObjectContext {
        container.viewContext
    }

    private init() {
        container = NSPersistentContainer(name: "Ramekin")

        // Use app group container for shared access with extensions
        if let appGroupURL = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: "group.com.ramekin.app"
        ) {
            let storeURL = appGroupURL.appendingPathComponent("Ramekin.sqlite")
            let storeDescription = NSPersistentStoreDescription(url: storeURL)
            container.persistentStoreDescriptions = [storeDescription]
        }

        container.loadPersistentStores { _, error in
            if let error = error as NSError? {
                // In a production app, handle this more gracefully
                fatalError("Failed to load Core Data stores: \(error), \(error.userInfo)")
            }
        }

        // Merge changes from background contexts automatically
        container.viewContext.automaticallyMergesChangesFromParent = true
        container.viewContext.mergePolicy = NSMergeByPropertyObjectTrumpMergePolicy
    }

    /// Creates a new background context for performing work off the main thread
    func newBackgroundContext() -> NSManagedObjectContext {
        let context = container.newBackgroundContext()
        context.mergePolicy = NSMergeByPropertyObjectTrumpMergePolicy
        return context
    }

    /// Saves the view context if there are unsaved changes
    func saveContext() {
        let context = viewContext
        if context.hasChanges {
            do {
                try context.save()
            } catch {
                let nsError = error as NSError
                print("CoreData save error: \(nsError), \(nsError.userInfo)")
            }
        }
    }

    /// Saves a background context if there are unsaved changes
    func save(context: NSManagedObjectContext) {
        if context.hasChanges {
            do {
                try context.save()
            } catch {
                let nsError = error as NSError
                print("CoreData save error: \(nsError), \(nsError.userInfo)")
            }
        }
    }
}

// MARK: - ShoppingItem Extensions

extension ShoppingItem {
    /// Sync status values
    enum SyncStatus: String {
        case synced = "synced"
        case pendingCreate = "pending_create"
        case pendingUpdate = "pending_update"
        case pendingDelete = "pending_delete"
    }

    var syncStatusEnum: SyncStatus {
        get {
            SyncStatus(rawValue: syncStatus ?? "synced") ?? .synced
        }
        set {
            syncStatus = newValue.rawValue
        }
    }

    /// Creates a new shopping item with default values
    static func create(
        in context: NSManagedObjectContext,
        item: String,
        amount: String? = nil,
        note: String? = nil,
        sourceRecipeId: UUID? = nil,
        sourceRecipeTitle: String? = nil,
        sortOrder: Int32 = 0
    ) -> ShoppingItem {
        let shoppingItem = ShoppingItem(context: context)
        shoppingItem.id = UUID()
        shoppingItem.item = item
        shoppingItem.amount = amount
        shoppingItem.note = note
        shoppingItem.sourceRecipeId = sourceRecipeId
        shoppingItem.sourceRecipeTitle = sourceRecipeTitle
        shoppingItem.isChecked = false
        shoppingItem.sortOrder = sortOrder
        shoppingItem.createdAt = Date()
        shoppingItem.updatedAt = Date()
        shoppingItem.syncStatus = SyncStatus.pendingCreate.rawValue
        shoppingItem.serverVersion = 0
        return shoppingItem
    }

    /// Marks the item as needing sync after a local update
    func markUpdated() {
        updatedAt = Date()
        if syncStatusEnum == .synced {
            syncStatusEnum = .pendingUpdate
        }
    }

    /// Marks the item for deletion (will be removed on next sync)
    func markDeleted() {
        syncStatusEnum = .pendingDelete
        updatedAt = Date()
    }

    /// Marks the item as synced with the server
    func markSynced(serverVersion: Int32) {
        self.serverVersion = serverVersion
        syncStatusEnum = .synced
    }
}

// MARK: - Fetch Requests

extension ShoppingItem {
    /// Fetches all items that are not pending deletion, sorted by checked status then sort order
    static func fetchActiveItems() -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(format: "syncStatus != %@", SyncStatus.pendingDelete.rawValue)
        request.sortDescriptors = [
            NSSortDescriptor(keyPath: \ShoppingItem.isChecked, ascending: true),
            NSSortDescriptor(keyPath: \ShoppingItem.sortOrder, ascending: true),
            NSSortDescriptor(keyPath: \ShoppingItem.createdAt, ascending: true)
        ]
        return request
    }

    /// Fetches all items that need to be synced to the server
    static func fetchPendingSync() -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(format: "syncStatus != %@", SyncStatus.synced.rawValue)
        return request
    }

    /// Fetches items pending deletion
    static func fetchPendingDelete() -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(format: "syncStatus == %@", SyncStatus.pendingDelete.rawValue)
        return request
    }

    /// Fetches an item by its UUID
    static func fetchById(_ id: UUID) -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(format: "id == %@", id as CVarArg)
        request.fetchLimit = 1
        return request
    }

    /// Fetches an item by its item name (case-insensitive)
    static func fetchByItemName(_ name: String) -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(
            format: "item ==[c] %@ AND syncStatus != %@",
            name,
            SyncStatus.pendingDelete.rawValue
        )
        return request
    }
}
