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
    /// The same plans by their `yyyy-MM-dd` day, for the week view.
    @Published private(set) var plansByDay: [String: [MealPlanItem]] = [:]
    @Published private(set) var isSyncing = false
    @Published private(set) var isOnline = true
    @Published var lastSyncError: String?
    /// Why the server refused a change made offline, to show once.
    @Published var refusal: String?

    private let coreDataStack: CoreDataStack
    private let userDefaults: UserDefaults
    private let syncMealPlans: (_ cursor: Int64?, _ ops: [[String: Any]]) async throws -> SyncMealPlansResponse
    private let automaticallySync: Bool
    private var activeAccountKey: String?
    /// Runs one sync pass at a time; any trigger during a pass queues a
    /// rerun.
    private var syncCoordinator: SyncCoordinator!

    private static let syncCursor = SyncCursorStore(prefix: "meal_plan_sync_cursor")

    init(
        coreDataStack: CoreDataStack = .shared,
        userDefaults: UserDefaults = .standard,
        initialAccountKey: String? = AccountScope.currentAccountKey(),
        automaticallySync: Bool = true,
        networkMonitor: NetworkMonitor = .shared,
        syncMealPlans: @escaping (_ cursor: Int64?, _ ops: [[String: Any]]) async throws -> SyncMealPlansResponse = {
            try await RamekinAPI.shared.syncMealPlans(cursor: $0, ops: $1)
        }
    ) {
        self.coreDataStack = coreDataStack
        self.userDefaults = coreDataStack.syncStateDefaults(userDefaults)
        self.automaticallySync = automaticallySync
        self.syncMealPlans = syncMealPlans
        syncCoordinator = SyncCoordinator { [weak self] in await self?.runSyncPass() }
        syncCoordinator.onBusyChanged = { [weak self] busy in self?.isSyncing = busy }
        if automaticallySync {
            isOnline = networkMonitor.isOnline
            networkMonitor.observe { [weak self] online in
                self?.isOnline = online
                if online {
                    Task { await self?.syncIfPending() }
                }
            }
        }
        setActiveAccountKey(initialAccountKey)
    }
}

extension MealPlanStore {
    // MARK: - Local Operations

    func setActiveAccountKey(_ accountKey: String?) {
        activeAccountKey = accountKey
        plans = []
        plansByDay = [:]
        lastSyncError = nil
        refusal = nil
        guard accountKey != nil else { return }
        fetchPlans()
        triggerSync()
    }

    func fetchPlans() {
        guard let activeAccountKey else {
            plans = []
            plansByDay = [:]
            return
        }
        let cached = (try? coreDataStack.viewContext.fetch(CachedMealPlan.fetchActive(accountKey: activeAccountKey))) ?? []
        plans = cached.compactMap(\.item)
        plansByDay = Dictionary(grouping: plans) { MealPlanDateSupport.apiDay(fromAPIDate: $0.mealDate) }
    }

    /// The plans for one meal on `date`, a local day.
    func plans(on date: Date, mealType: MealType) -> [MealPlanItem] {
        let day = SharedDateFormatters.localDateOnly.string(from: date)
        return plansByDay[day, default: []].filter { $0.mealType == mealType }
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
        plan.markDeleted()
        try? saveAndSync()
    }

    private func isPlanned(recipeId: UUID, day: String, mealType: MealType, except: UUID?) -> Bool {
        plansByDay[day, default: []].contains { plan in
            plan.id != except && plan.recipeId == recipeId && plan.mealType == mealType
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
        guard automaticallySync, isOnline else { return }
        syncCoordinator.request()
    }

    /// Sends edits made offline; a no-op when there are none. During a sync
    /// it queues a rerun instead.
    func syncIfPending() async {
        guard let activeAccountKey, isOnline else { return }
        guard !syncCoordinator.isBusy else {
            syncCoordinator.request()
            return
        }
        guard hasPendingEdits(accountKey: activeAccountKey) else { return }
        await syncCoordinator.sync()
    }

    /// Sync now, and wait for it. When a sync is already running this waits
    /// for it and for one more pass after it, so pull-to-refresh always
    /// ends with a pass that started after the pull.
    func syncWithServer() async {
        await syncCoordinator.sync()
    }

    /// One round trip: send the outbox, apply the response. Only the
    /// coordinator calls this, so passes never overlap.
    private func runSyncPass() async {
        let logger = DebugLogger.shared
        guard let syncAccountKey = activeAccountKey, isOnline else { return }
        lastSyncError = nil

        do {
            let pending = try coreDataStack.viewContext.fetch(
                CachedMealPlan.fetchPendingSync(accountKey: syncAccountKey)
            )
            let outbox = MealPlanSyncSupport.outbox(for: pending)
            let response = try await logger.timed("meal plan sync API", source: "MealPlan") {
                try await syncMealPlans(Self.syncCursor.cursor(accountKey: syncAccountKey, in: userDefaults), outbox.ops)
            }
            process(response, outbox: outbox, accountKey: syncAccountKey)
            Self.syncCursor.set(response.cursor, accountKey: syncAccountKey, in: userDefaults)
        } catch {
            logger.log("meal plan sync failed: \(error.localizedDescription)", source: "MealPlan")
            if activeAccountKey == syncAccountKey {
                lastSyncError = error.localizedDescription
            }
        }

        fetchPlans()
    }

    private func hasPendingEdits(accountKey: String) -> Bool {
        (try? coreDataStack.viewContext.fetch(
            CachedMealPlan.fetchPendingSync(accountKey: accountKey)
        ))?.isEmpty == false
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
            if activeAccountKey == accountKey,
               let message = Self.refusalMessage(result, kind: sent.kind, recipeTitle: plan.recipeTitle ?? "A recipe") {
                refusal = message
            }
            if plan.settle(result: result, sent: sent) == .removed {
                context.delete(plan)
            }
        }

        MealPlanSyncSupport.apply(response, accountKey: accountKey, in: context)
        coreDataStack.saveContext()
    }

    /// What to tell the user about an op the server didn't apply, or nil
    /// when there's nothing to say: an applied op, or a delete of a plan
    /// that was already gone.
    static func refusalMessage(_ result: OpResult, kind: OpKind, recipeTitle: String) -> String? {
        switch result {
        case .applied:
            return nil
        case .recipeMissing:
            return "\(recipeTitle) was deleted, so it was taken off the meal plan."
        case .duplicate:
            return "\(recipeTitle) was already planned for that meal, so the change was undone."
        case .gone:
            guard kind != .delete else { return nil }
            return "\(recipeTitle) was already removed from the meal plan, so the change was dropped."
        }
    }
}
