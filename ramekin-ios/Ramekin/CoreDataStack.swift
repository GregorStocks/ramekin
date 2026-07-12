import CoreData
import Foundation

/// Manages the Core Data stack for offline shopping list storage
class CoreDataStack: ObservableObject {
    static let shared = CoreDataStack()
    private let logger = DebugLogger.shared

    private static let modelName = "Ramekin"

    /// The one and only managed-object model instance, shared by every container.
    ///
    /// `NSPersistentContainer(name:)` loads a fresh model from the bundle on each call. Two live
    /// models both claim `CachedRecipe` and `ShoppingItem`, and Core Data then resolves `+entity`
    /// (which backs `ShoppingItem(context:)`) against an arbitrary one of them. Loading the model
    /// exactly once keeps a single entity description per subclass.
    static let managedObjectModel: NSManagedObjectModel = {
        let name = CoreDataStack.modelName
        guard let url = Bundle(for: CoreDataStack.self).url(forResource: name, withExtension: "momd"),
            let model = NSManagedObjectModel(contentsOf: url)
        else {
            fatalError("Failed to load the \(name) Core Data model")
        }
        return model
    }()

    /// Builds a container backed by the shared model. Callers configure the store descriptions
    /// and load the stores themselves.
    static func makeContainer() -> NSPersistentContainer {
        NSPersistentContainer(name: modelName, managedObjectModel: managedObjectModel)
    }

    let container: NSPersistentContainer

    /// The main view context for UI operations
    var viewContext: NSManagedObjectContext {
        container.viewContext
    }

    private init() {
        container = CoreDataStack.makeContainer()

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

        configureViewContext()
    }

    init(container: NSPersistentContainer) {
        self.container = container
        configureViewContext()
    }

    private func configureViewContext() {
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
        do {
            try saveContextOrThrow()
        } catch {
            let nsError = error as NSError
            logger.log(
                "CoreData save error: \(nsError), \(nsError.userInfo)",
                source: "CoreDataStack"
            )
        }
    }

    /// Saves the view context if there are unsaved changes and surfaces failures to callers.
    func saveContextOrThrow() throws {
        let context = viewContext
        if context.hasChanges {
            try context.save()
        }
    }

    /// Saves a background context if there are unsaved changes
    func save(context: NSManagedObjectContext) {
        if context.hasChanges {
            do {
                try context.save()
            } catch {
                let nsError = error as NSError
                logger.log(
                    "CoreData save error: \(nsError), \(nsError.userInfo)",
                    source: "CoreDataStack"
                )
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
        accountKey: String,
        item: String,
        amount: String? = nil,
        note: String? = nil,
        sourceRecipeId: UUID? = nil,
        sourceRecipeTitle: String? = nil,
        sortOrder: Int32 = 0
    ) -> ShoppingItem {
        let shoppingItem = ShoppingItem(context: context)
        shoppingItem.accountKey = accountKey
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
        shoppingItem.clearCategoryOverride = false
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
        clearCategoryOverride = false
        syncStatusEnum = .synced
    }
}

// MARK: - Fetch Requests

extension ShoppingItem {
    /// Fetches all items that are not pending deletion, sorted by checked status then sort order
    static func fetchActiveItems(accountKey: String) -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(
            format: "accountKey == %@ AND syncStatus != %@",
            accountKey,
            SyncStatus.pendingDelete.rawValue
        )
        request.sortDescriptors = [
            NSSortDescriptor(keyPath: \ShoppingItem.isChecked, ascending: true),
            NSSortDescriptor(keyPath: \ShoppingItem.sortOrder, ascending: true),
            NSSortDescriptor(keyPath: \ShoppingItem.createdAt, ascending: true)
        ]
        return request
    }

    /// Fetches all items that need to be synced to the server
    static func fetchPendingSync(accountKey: String) -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(
            format: "accountKey == %@ AND syncStatus != %@",
            accountKey,
            SyncStatus.synced.rawValue
        )
        return request
    }

    /// Fetches items pending deletion
    static func fetchPendingDelete(accountKey: String) -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(
            format: "accountKey == %@ AND syncStatus == %@",
            accountKey,
            SyncStatus.pendingDelete.rawValue
        )
        return request
    }

    /// Fetches an item by its UUID
    static func fetchById(_ id: UUID, accountKey: String) -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(
            format: "accountKey == %@ AND id == %@",
            accountKey,
            id as CVarArg
        )
        request.fetchLimit = 1
        return request
    }

    /// Fetches an item by its item name (case-insensitive)
    static func fetchByItemName(_ name: String, accountKey: String) -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(
            format: "accountKey == %@ AND item ==[c] %@ AND syncStatus != %@",
            accountKey,
            name,
            SyncStatus.pendingDelete.rawValue
        )
        return request
    }

    static func fetchUnscopedItems() -> NSFetchRequest<ShoppingItem> {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(format: "accountKey == nil OR accountKey == ''")
        return request
    }
}
