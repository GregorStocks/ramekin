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
        var sent: [UUID: OutboxSent] = [:]
    }

    static func outbox(for pending: [ShoppingItem]) -> Outbox {
        var outbox = Outbox()
        for item in pending {
            guard let id = item.id else { continue }
            let operation: ShoppingListOp
            switch item.syncStatusEnum {
            case .pendingCreate, .pendingRestore:
                // A restore sets every field, so "" clears the empty ones.
                let restoring = item.syncStatusEnum == .pendingRestore
                func text(_ value: String?) -> String? {
                    restoring ? (value ?? "") : value
                }
                operation = ShoppingListOp(
                    fields: ShoppingListFields(
                        amount: text(item.amount),
                        categoryOverride: text(item.categoryOverride),
                        isChecked: item.isChecked,
                        item: item.item ?? "",
                        note: text(item.note),
                        sortOrder: Int(item.sortOrder),
                        sourceRecipeId: item.sourceRecipeId,
                        sourceRecipeTitle: item.sourceRecipeTitle
                    ),
                    id: id,
                    op: restoring ? .restore : .create
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
            outbox.sent[id] = item.sent(operation.op)
        }
        return outbox
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
        OutboxSupport.apply(
            response,
            cached: (try? context.fetch(request)) ?? [],
            in: context,
            insert: { newItem(id: $0, accountKey: accountKey, in: context) },
            take: take(_:into:)
        )
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
    }
}
