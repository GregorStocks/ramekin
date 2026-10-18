import CoreData
import Foundation

/// Meal plans, cached offline in Core Data with an outbox of local edits
/// that syncs through `POST /api/sync/meal-plans` (doc/sync-design.md).
@MainActor
final class MealPlanStore: ObservableObject {
    enum MealPlanError: LocalizedError, Equatable {
        case noActiveAccount
        case alreadyPlanned

        var errorDescription: String? {
            switch self {
            case .noActiveAccount:
                return "Sign in to plan meals."
            case .alreadyPlanned:
                return "This recipe is already planned for this meal."
            }
        }
    }

    static let shared = MealPlanStore()

    /// Every plan not waiting to be deleted, for the active account.
    @Published private(set) var plans: [MealPlanItem] = []
    @Published private(set) var isSyncing = false
    @Published var lastSyncError: String?
    /// Why the server refused a change made offline, to show once.
    @Published var refusal: String?

    private let coreDataStack: CoreDataStack
    private let userDefaults: UserDefaults
    private let syncMealPlans: (_ cursor: Int64?, _ ops: [[String: Any]]) async throws -> SyncMealPlansResponse
    private let automaticallySync: Bool
    private var activeAccountKey: String?

    private static let syncCursorKeyPrefix = "meal_plan_sync_cursor"

    init(
        coreDataStack: CoreDataStack = .shared,
        userDefaults: UserDefaults = .standard,
        initialAccountKey: String? = AccountScope.currentAccountKey(),
        automaticallySync: Bool = true,
        syncMealPlans: @escaping (_ cursor: Int64?, _ ops: [[String: Any]]) async throws -> SyncMealPlansResponse = {
            try await RamekinAPI.shared.syncMealPlans(cursor: $0, ops: $1)
        }
    ) {
        self.coreDataStack = coreDataStack
        self.userDefaults = coreDataStack.syncStateDefaults(userDefaults)
        self.automaticallySync = automaticallySync
        self.syncMealPlans = syncMealPlans
        setActiveAccountKey(initialAccountKey)
    }
}

extension MealPlanStore {
    // MARK: - Local Operations

    func setActiveAccountKey(_ accountKey: String?) {
        activeAccountKey = accountKey
        plans = []
        lastSyncError = nil
        refusal = nil
        guard accountKey != nil else { return }
        fetchPlans()
        triggerSync()
    }

    func fetchPlans() {
        guard let activeAccountKey else {
            plans = []
            return
        }
        let cached = (try? coreDataStack.viewContext.fetch(CachedMealPlan.fetchActive(accountKey: activeAccountKey))) ?? []
        plans = cached.compactMap(\.item)
    }

    /// Plan `recipe` for the meal on `date`, a local day.
    func add(
        recipeId: UUID,
        recipeTitle: String,
        thumbnailPhotoId: UUID?,
        date: Date,
        mealType: MealType,
        notes: String? = nil
    ) throws {
        guard let activeAccountKey else { throw MealPlanError.noActiveAccount }
        let day = SharedDateFormatters.localDateOnly.string(from: date)
        guard !isPlanned(recipeId: recipeId, day: day, mealType: mealType, except: nil) else {
            throw MealPlanError.alreadyPlanned
        }
        let plan = CachedMealPlan(context: coreDataStack.viewContext)
        plan.accountKey = activeAccountKey
        plan.id = UUID()
        plan.recipeId = recipeId
        plan.recipeTitle = recipeTitle
        plan.thumbnailPhotoId = thumbnailPhotoId
        plan.mealDay = day
        plan.mealType = mealType.rawValue
        plan.notes = notes.flatMap { $0.isEmpty ? nil : $0 }
        plan.changeSeq = 0
        plan.syncStatusEnum = .pendingCreate
        try saveAndSync()
    }

    func update(_ meal: MealPlanItem, date: Date, mealType: MealType, notes: String) throws {
        guard let plan = cachedPlan(id: meal.id) else { return }
        let day = SharedDateFormatters.localDateOnly.string(from: date)
        guard let recipeId = plan.recipeId,
              !isPlanned(recipeId: recipeId, day: day, mealType: mealType, except: meal.id) else {
            throw MealPlanError.alreadyPlanned
        }
        var fields: Set<String> = []
        if plan.mealDay != day {
            plan.mealDay = day
            fields.insert("meal_date")
        }
        if plan.mealType != mealType.rawValue {
            plan.mealType = mealType.rawValue
            fields.insert("meal_type")
        }
        let newNotes = notes.isEmpty ? nil : notes
        if plan.notes != newNotes {
            plan.notes = newNotes
            fields.insert("notes")
        }
        guard !fields.isEmpty else { return }
        plan.markUpdated(fields)
        try saveAndSync()
    }

    /// Deleted through the server even when its create looks unsent: the
    /// create may have reached it on a sync whose response was lost.
    func delete(_ meal: MealPlanItem) {
        guard let plan = cachedPlan(id: meal.id) else { return }
        plan.syncStatusEnum = .pendingDelete
        plan.dirtyFields = nil
        try? saveAndSync()
    }

    private func isPlanned(recipeId: UUID, day: String, mealType: MealType, except: UUID?) -> Bool {
        guard let activeAccountKey else { return false }
        let active = (try? coreDataStack.viewContext.fetch(CachedMealPlan.fetchActive(accountKey: activeAccountKey))) ?? []
        return active.contains { plan in
            plan.id != except && plan.recipeId == recipeId && plan.mealDay == day && plan.mealType == mealType.rawValue
        }
    }

    private func cachedPlan(id: UUID) -> CachedMealPlan? {
        guard let activeAccountKey else { return nil }
        let request = CachedMealPlan.fetchAll(accountKey: activeAccountKey)
        request.predicate = NSPredicate(format: "accountKey == %@ AND id == %@", activeAccountKey, id as CVarArg)
        request.fetchLimit = 1
        return (try? coreDataStack.viewContext.fetch(request))?.first
    }

    private func saveAndSync() throws {
        do {
            try coreDataStack.saveContextOrThrow()
        } catch {
            coreDataStack.viewContext.rollback()
            throw error
        }
        fetchPlans()
        triggerSync()
    }

    // MARK: - Sync

    private func triggerSync() {
        guard automaticallySync else { return }
        Task { await syncWithServer() }
    }

    func syncWithServer(isFollowUp: Bool = false) async {
        let logger = DebugLogger.shared
        guard let syncAccountKey = activeAccountKey, !isSyncing else { return }
        isSyncing = true
        lastSyncError = nil

        do {
            let pending = try coreDataStack.viewContext.fetch(
                CachedMealPlan.fetchPendingSync(accountKey: syncAccountKey)
            )
            let outbox = MealPlanSyncSupport.outbox(for: pending)
            let response = try await logger.timed("meal plan sync API", source: "MealPlan") {
                try await syncMealPlans(syncCursor(accountKey: syncAccountKey), outbox.ops)
            }
            process(response, outbox: outbox, accountKey: syncAccountKey)
            setSyncCursor(response.cursor, accountKey: syncAccountKey)
        } catch {
            logger.log("meal plan sync failed: \(error.localizedDescription)", source: "MealPlan")
            if activeAccountKey == syncAccountKey {
                lastSyncError = error.localizedDescription
            }
        }

        isSyncing = false
        fetchPlans()

        // Edits made while the sync was in flight go out in one follow-up.
        guard activeAccountKey == syncAccountKey, lastSyncError == nil, !isFollowUp else { return }
        let hasPending = (try? coreDataStack.viewContext.fetch(
            CachedMealPlan.fetchPendingSync(accountKey: syncAccountKey)
        ))?.isEmpty == false
        if hasPending {
            await syncWithServer(isFollowUp: true)
        }
    }

    private func process(_ response: SyncMealPlansResponse, outbox: MealPlanSyncSupport.Outbox, accountKey: String) {
        let context = coreDataStack.viewContext
        let byId = Dictionary(
            ((try? context.fetch(CachedMealPlan.fetchAll(accountKey: accountKey))) ?? []).compactMap { plan in
                plan.id.map { ($0, plan) }
            },
            uniquingKeysWith: { first, _ in first }
        )
        for (id, result) in zip(outbox.order, response.results) {
            guard let plan = byId[id], let sent = outbox.sent[id] else { continue }
            switch MealPlanSyncSupport.settle(plan, result: result, sent: sent) {
            case .kept:
                break
            case .removed:
                context.delete(plan)
            case .refused(let result):
                if activeAccountKey == accountKey {
                    refusal = Self.refusalMessage(result, recipeTitle: plan.recipeTitle ?? "A recipe")
                }
                if sent.kind == .create {
                    context.delete(plan)
                }
            }
        }

        MealPlanSyncSupport.apply(response, accountKey: accountKey, in: context)
        coreDataStack.saveContext()
    }

    static func refusalMessage(_ result: OpResult, recipeTitle: String) -> String {
        switch result {
        case .recipeMissing:
            return "\(recipeTitle) was deleted, so it was taken off the meal plan."
        default:
            return "\(recipeTitle) was already planned for that meal, so the change was undone."
        }
    }

    private func syncCursor(accountKey: String) -> Int64? {
        userDefaults.object(forKey: syncCursorKey(accountKey: accountKey)) as? Int64
    }

    private func setSyncCursor(_ cursor: Int64, accountKey: String) {
        userDefaults.set(cursor, forKey: syncCursorKey(accountKey: accountKey))
    }

    private func syncCursorKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(prefix: Self.syncCursorKeyPrefix, accountKey: accountKey)
    }
}
