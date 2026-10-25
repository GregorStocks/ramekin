import CoreData
import Foundation

extension CachedMealPlan: OutboxRow {
    var rowId: UUID? {
        get { id }
        set { id = newValue }
    }

    /// API names of the fields an update op can send.
    static let syncedFields = ["meal_date", "meal_type", "notes"]

    func syncedValue(of field: String) -> String? {
        switch field {
        case "meal_date": return mealDay
        case "meal_type": return mealType
        case "notes": return notes
        default: return nil
        }
    }

    /// The plan as the views show it.
    var item: MealPlanItem? {
        guard let id, let recipeId, let mealDay, let recipeTitle,
              let mealDate = MealPlanDateSupport.apiDate(fromDay: mealDay),
              let mealType = mealType.flatMap(MealType.init(rawValue:)) else {
            return nil
        }
        return MealPlanItem(
            id: id,
            mealDate: mealDate,
            mealType: mealType,
            notes: notes,
            recipeId: recipeId,
            recipeTitle: recipeTitle,
            thumbnailPhotoId: thumbnailPhotoId
        )
    }

    static func fetchAll(accountKey: String) -> NSFetchRequest<CachedMealPlan> {
        let request = NSFetchRequest<CachedMealPlan>(entityName: "CachedMealPlan")
        request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
        return request
    }

    /// Plans to show: everything not waiting to be deleted.
    static func fetchActive(accountKey: String) -> NSFetchRequest<CachedMealPlan> {
        let request = NSFetchRequest<CachedMealPlan>(entityName: "CachedMealPlan")
        request.predicate = NSPredicate(
            format: "accountKey == %@ AND syncStatus != %@",
            accountKey,
            OutboxStatus.pendingDelete.rawValue
        )
        request.sortDescriptors = [
            NSSortDescriptor(keyPath: \CachedMealPlan.mealDay, ascending: true),
            NSSortDescriptor(keyPath: \CachedMealPlan.recipeTitle, ascending: true)
        ]
        return request
    }

    static func fetchPendingSync(accountKey: String) -> NSFetchRequest<CachedMealPlan> {
        let request = NSFetchRequest<CachedMealPlan>(entityName: "CachedMealPlan")
        request.predicate = NSPredicate(
            format: "accountKey == %@ AND syncStatus != %@",
            accountKey,
            OutboxStatus.synced.rawValue
        )
        return request
    }
}

/// The meal plan's outbox, the same protocol as the shopping list's
/// (doc/sync-design.md).
enum MealPlanSyncSupport {
    struct Outbox {
        /// Op objects for the request body (see `RamekinAPI.syncMealPlans`).
        var ops: [[String: Any]] = []
        var sent: [UUID: OutboxSent] = [:]
        /// Op ids in request order, matching the response's results.
        var order: [UUID] = []
    }

    static func outbox(for pending: [CachedMealPlan]) -> Outbox {
        var outbox = Outbox()
        for plan in pending {
            guard let id = plan.id else { continue }
            let kind: OpKind
            var fields: [String: Any] = [:]
            switch plan.syncStatusEnum {
            case .pendingCreate:
                kind = .create
                fields["recipe_id"] = plan.recipeId?.uuidString
                fields["meal_date"] = plan.mealDay
                fields["meal_type"] = plan.mealType
                fields["notes"] = plan.notes
            case .pendingUpdate:
                kind = .update
                for field in plan.dirtyFieldSet {
                    // "" clears notes.
                    fields[field] = plan.syncedValue(of: field) ?? ""
                }
            case .pendingDelete:
                kind = .delete
            case .synced, .pendingRestore:
                // Meal plans are never restored.
                continue
            }
            var operation: [String: Any] = ["op": kind.rawValue, "id": id.uuidString]
            if !fields.isEmpty {
                operation["fields"] = fields
            }
            outbox.ops.append(operation)
            outbox.order.append(id)
            outbox.sent[id] = plan.sent(kind)
        }
        return outbox
    }

    static func apply(
        _ response: SyncMealPlansResponse,
        accountKey: String,
        in context: NSManagedObjectContext
    ) {
        OutboxSupport.apply(
            response,
            cached: (try? context.fetch(CachedMealPlan.fetchAll(accountKey: accountKey))) ?? [],
            in: context,
            insert: { id in
                let plan = CachedMealPlan(context: context)
                plan.accountKey = accountKey
                plan.id = id
                plan.markSynced()
                return plan
            },
            take: take(_:into:)
        )
    }

    private static func take(_ change: SyncMealPlan, into plan: CachedMealPlan) {
        let dirty = plan.dirtyFieldSet
        if !dirty.contains("meal_date") { plan.mealDay = MealPlanDateSupport.apiDay(fromAPIDate: change.mealDate) }
        if !dirty.contains("meal_type") { plan.mealType = change.mealType.rawValue }
        if !dirty.contains("notes") { plan.notes = change.notes }
        plan.recipeId = change.recipeId
        plan.recipeTitle = change.recipeTitle
        plan.thumbnailPhotoId = change.thumbnailPhotoId
    }
}
