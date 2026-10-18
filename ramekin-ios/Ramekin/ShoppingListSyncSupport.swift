import CoreData
import Foundation

/// The shopping list's outbox: turning pending local writes into sync ops,
/// settling them from the op results, and applying the server's changes
/// (doc/sync-design.md). Writes are per-field last-writer-wins, so nothing
/// is ever rejected for being stale.
enum ShoppingListSyncSupport {
    /// The ops for every pending item, and what each op sent.
    struct Outbox {
        var ops: [ShoppingListOp] = []
        var sent: [UUID: Sent] = [:]
    }

    /// An op as sent: its kind, and the value of each field it carried, so
    /// a field edited again mid-sync stays dirty.
    struct Sent {
        let kind: OpKind
        let values: [String: String?]
    }

    static func outbox(for pending: [ShoppingItem]) -> Outbox {
        var outbox = Outbox()
        for item in pending {
            guard let id = item.id else { continue }
            let operation: ShoppingListOp
            switch item.syncStatusEnum {
            case .pendingCreate:
                operation = ShoppingListOp(
                    fields: ShoppingListFields(
                        amount: item.amount,
                        categoryOverride: item.categoryOverride,
                        isChecked: item.isChecked,
                        item: item.item ?? "",
                        note: item.note,
                        sortOrder: Int(item.sortOrder),
                        sourceRecipeId: item.sourceRecipeId,
                        sourceRecipeTitle: item.sourceRecipeTitle
                    ),
                    id: id,
                    op: .create
                )
            case .pendingUpdate:
                let dirty = item.dirtyFieldSet
                // "" clears a nullable field: the generated encoder drops nil.
                func text(_ field: String, _ value: String?) -> String? {
                    dirty.contains(field) ? (value ?? "") : nil
                }
                operation = ShoppingListOp(
                    fields: ShoppingListFields(
                        amount: text("amount", item.amount),
                        categoryOverride: text("category_override", item.categoryOverride),
                        isChecked: dirty.contains("is_checked") ? item.isChecked : nil,
                        item: dirty.contains("item") ? item.item : nil,
                        note: text("note", item.note),
                        sortOrder: dirty.contains("sort_order") ? Int(item.sortOrder) : nil
                    ),
                    id: id,
                    op: .update
                )
            case .pendingDelete:
                operation = ShoppingListOp(id: id, op: .delete)
            case .synced:
                continue
            }
            outbox.ops.append(operation)
            outbox.sent[id] = Sent(
                kind: operation.op,
                values: Dictionary(
                    uniqueKeysWithValues: item.dirtyFieldSet.map { ($0, item.syncedValue(of: $0)) }
                )
            )
        }
        return outbox
    }

    enum Settled: Equatable {
        case kept
        /// The server deleted the item, or the local delete went through.
        case removed
    }

    /// Settle an item's op from its result. A field that still holds the
    /// value sent is clean; one edited again while the sync was in flight
    /// stays dirty and goes out next round, and so does a delete made then.
    static func settle(_ item: ShoppingItem, result: OpResult, sent: Sent) -> Settled {
        if result == .gone || sent.kind == .delete {
            return .removed
        }
        switch item.syncStatusEnum {
        case .pendingDelete, .synced:
            // Deleted locally after its create or update went out: the
            // delete goes out next round.
            return .kept
        case .pendingCreate, .pendingUpdate:
            let stillDirty = item.dirtyFieldSet.filter { field in
                guard let value = sent.values[field] else { return true }
                return item.syncedValue(of: field) != value
            }
            if stillDirty.isEmpty {
                item.markSynced()
            } else {
                item.syncStatusEnum = .pendingUpdate
                item.dirtyFieldSet = stillDirty
            }
            return .kept
        }
    }

    /// Apply the server's changes and tombstones to every cached item of
    /// the account, keeping fields with unsent edits.
    static func apply(
        _ response: SyncShoppingListResponse,
        accountKey: String,
        in context: NSManagedObjectContext
    ) {
        let request = NSFetchRequest<ShoppingItem>(entityName: "ShoppingItem")
        request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
        let cachedItems = (try? context.fetch(request)) ?? []
        var byId: [UUID: ShoppingItem] = [:]
        for item in cachedItems {
            if let id = item.id { byId[id] = item }
        }
        let changes = Dictionary(response.changes.map { ($0.id, $0) }, uniquingKeysWith: { $1 })

        let actions = SyncApplySupport.plan(
            cached: byId.mapValues {
                SyncApplySupport.CachedState(changeSeq: $0.changeSeq, pending: $0.pendingWrite)
            },
            changes: response.changes.map { (id: $0.id, changeSeq: $0.changeSeq) },
            deleted: response.deleted,
            fullResync: response.fullResync
        )
        for (id, action) in actions {
            switch action {
            case .keep:
                break
            case .drop:
                if let item = byId[id] { context.delete(item) }
            case .take:
                guard let change = changes[id] else { continue }
                let item = byId[id] ?? newItem(id: id, accountKey: accountKey, in: context)
                take(change, into: item)
            }
        }
    }

    private static func newItem(id: UUID, accountKey: String, in context: NSManagedObjectContext) -> ShoppingItem {
        let item = ShoppingItem(context: context)
        item.accountKey = accountKey
        item.id = id
        item.createdAt = Date()
        item.updatedAt = Date()
        item.markSynced()
        return item
    }

    private static func take(_ change: SyncShoppingListItem, into item: ShoppingItem) {
        let dirty = item.dirtyFieldSet
        if !dirty.contains("item") { item.item = change.item }
        if !dirty.contains("amount") { item.amount = change.amount }
        if !dirty.contains("note") { item.note = change.note }
        if !dirty.contains("is_checked") { item.isChecked = change.isChecked }
        if !dirty.contains("sort_order") { item.sortOrder = Int32(change.sortOrder) }
        if !dirty.contains("category_override") {
            item.categoryOverride = change.categoryOverride
            item.category = change.category
        } else {
            item.category = item.categoryOverride ?? change.computedCategory
        }
        item.sourceRecipeId = change.sourceRecipeId
        item.sourceRecipeTitle = change.sourceRecipeTitle
        item.computedCategory = change.computedCategory
        item.changeSeq = change.changeSeq
    }
}
