import CoreData
import Foundation

extension CachedMealPlan {
    enum SyncStatus: String {
        case synced
        case pendingCreate = "pending_create"
        case pendingUpdate = "pending_update"
        case pendingDelete = "pending_delete"
    }

    /// API names of the fields an update op can send.
    static let syncedFields = ["meal_date", "meal_type", "notes"]

    var syncStatusEnum: SyncStatus {
        get { SyncStatus(rawValue: syncStatus ?? "synced") ?? .synced }
        set { syncStatus = newValue.rawValue }
    }

    var pendingWrite: SyncApplySupport.PendingWrite? {
        switch syncStatusEnum {
        case .synced: return nil
        case .pendingCreate: return .create
        case .pendingUpdate: return .update
        case .pendingDelete: return .delete
        }
    }

    /// The fields with unsent local edits: every field of a pending create,
    /// none of a synced plan or pending delete.
    var dirtyFieldSet: Set<String> {
        get {
            switch syncStatusEnum {
            case .pendingCreate:
                return Set(Self.syncedFields)
            case .pendingUpdate:
                return DirtyFields.decode(dirtyFields, allFields: Self.syncedFields)
            case .synced, .pendingDelete:
                return []
            }
        }
        set { dirtyFields = DirtyFields.encode(newValue) }
    }

    func syncedValue(of field: String) -> String? {
        switch field {
        case "meal_date": return mealDay
        case "meal_type": return mealType
        case "notes": return notes
        default: return nil
        }
    }

    func markUpdated(_ fields: Set<String>) {
        switch syncStatusEnum {
        case .synced:
            dirtyFieldSet = fields
            syncStatusEnum = .pendingUpdate
        case .pendingUpdate:
            dirtyFieldSet = dirtyFieldSet.union(fields)
        case .pendingCreate, .pendingDelete:
            break
        }
    }

    func markSynced() {
        dirtyFields = nil
        syncStatusEnum = .synced
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
            SyncStatus.pendingDelete.rawValue
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
            SyncStatus.synced.rawValue
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
        var sent: [UUID: Sent] = [:]
        /// Op ids in request order, matching the response's results.
        var order: [UUID] = []
    }

    struct Sent {
        let kind: OpKind
        let values: [String: String?]
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
            case .synced:
                continue
            }
            var operation: [String: Any] = ["op": kind.rawValue, "id": id.uuidString]
            if !fields.isEmpty {
                operation["fields"] = fields
            }
            outbox.ops.append(operation)
            outbox.order.append(id)
            outbox.sent[id] = Sent(
                kind: kind,
                values: Dictionary(
                    uniqueKeysWithValues: plan.dirtyFieldSet.map { ($0, plan.syncedValue(of: $0)) }
                )
            )
        }
        return outbox
    }

    enum Settled: Equatable {
        case kept
        case removed
        /// The server refused the op; the plan is dropped (a create) or
        /// retakes the server's copy (an update).
        case refused(OpResult)
    }

    static func settle(_ plan: CachedMealPlan, result: OpResult, sent: Sent) -> Settled {
        switch result {
        case .gone:
            return .removed
        case .duplicate, .recipeMissing:
            guard sent.kind == .update else { return .refused(result) }
            // The server's copy comes back in the same response.
            plan.markSynced()
            return .refused(result)
        case .applied:
            break
        }
        if sent.kind == .delete {
            return .removed
        }
        switch plan.syncStatusEnum {
        case .pendingDelete, .synced:
            return .kept
        case .pendingCreate, .pendingUpdate:
            let stillDirty = plan.dirtyFieldSet.filter { field in
                guard let value = sent.values[field] else { return true }
                return plan.syncedValue(of: field) != value
            }
            if stillDirty.isEmpty {
                plan.markSynced()
            } else {
                plan.syncStatusEnum = .pendingUpdate
                plan.dirtyFieldSet = stillDirty
            }
            return .kept
        }
    }

    static func apply(
        _ response: SyncMealPlansResponse,
        accountKey: String,
        in context: NSManagedObjectContext
    ) {
        let cachedPlans = (try? context.fetch(CachedMealPlan.fetchAll(accountKey: accountKey))) ?? []
        var byId: [UUID: CachedMealPlan] = [:]
        for plan in cachedPlans {
            if let id = plan.id { byId[id] = plan }
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
                if let plan = byId[id] { context.delete(plan) }
            case .take:
                guard let change = changes[id] else { continue }
                let plan = byId[id] ?? {
                    let plan = CachedMealPlan(context: context)
                    plan.accountKey = accountKey
                    plan.id = id
                    plan.markSynced()
                    return plan
                }()
                take(change, into: plan)
            }
        }
    }

    private static func take(_ change: SyncMealPlan, into plan: CachedMealPlan) {
        let dirty = plan.dirtyFieldSet
        if !dirty.contains("meal_date") { plan.mealDay = MealPlanDateSupport.apiDay(fromAPIDate: change.mealDate) }
        if !dirty.contains("meal_type") { plan.mealType = change.mealType.rawValue }
        if !dirty.contains("notes") { plan.notes = change.notes }
        plan.recipeId = change.recipeId
        plan.recipeTitle = change.recipeTitle
        plan.thumbnailPhotoId = change.thumbnailPhotoId
        plan.changeSeq = change.changeSeq
    }
}
