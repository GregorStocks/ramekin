import CoreData
import XCTest
@testable import Ramekin

@MainActor
final class MealPlanStoreTests: XCTestCase {
    private typealias Request = (cursor: Int64?, ops: [[String: Any]])

    func testAPlanAddedOfflineShowsAtOnceAndSyncsAsACreate() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [Request] = []
        var responses: [SyncMealPlansResponse] = []
        let store = makeStore(stack: stack, defaults: defaults) { cursor, ops in
            requests.append((cursor, ops))
            return responses.removeFirst()
        }
        let recipeId = UUID()

        try store.add(recipeId: recipeId, recipeTitle: "Soup", thumbnailPhotoId: nil, date: day("2026-04-17"), mealType: .dinner)

        let plan = try XCTUnwrap(store.plans.first)
        XCTAssertEqual(plan.recipeTitle, "Soup")
        XCTAssertEqual(MealPlanDateSupport.apiDay(fromAPIDate: plan.mealDate), "2026-04-17")
        responses.append(response(results: [.applied], changes: [change(id: plan.id, recipeId: recipeId, title: "Soup")]))
        await store.syncWithServer(isFollowUp: true)

        let create = try XCTUnwrap(requests.first?.ops.first)
        XCTAssertEqual(create["op"] as? String, "create")
        XCTAssertEqual(create["id"] as? String, plan.id.uuidString)
        let fields = try XCTUnwrap(create["fields"] as? [String: Any])
        XCTAssertEqual(fields["meal_date"] as? String, "2026-04-17")
        XCTAssertEqual(fields["meal_type"] as? String, "dinner")
        XCTAssertEqual(fields["recipe_id"] as? String, recipeId.uuidString)
        XCTAssertNil(requests.first?.cursor)
        XCTAssertTrue(try stack.viewContext.fetch(CachedMealPlan.fetchPendingSync(accountKey: account)).isEmpty)
    }

    func testPlanningARecipeTwiceForOneMealIsRefusedLocally() throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let store = makeStore(stack: stack, defaults: defaults) { _, _ in self.response() }
        let recipeId = UUID()

        try store.add(recipeId: recipeId, recipeTitle: "Soup", thumbnailPhotoId: nil, date: day("2026-04-17"), mealType: .dinner)

        XCTAssertThrowsError(
            try store.add(recipeId: recipeId, recipeTitle: "Soup", thumbnailPhotoId: nil, date: day("2026-04-17"), mealType: .dinner)
        ) { error in
            XCTAssertEqual(error as? MealPlanStore.MealPlanError, .alreadyPlanned)
        }
        XCTAssertEqual(store.plans.count, 1)
    }

    func testAnUpdateSendsOnlyTheChangedFields() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [Request] = []
        let id = UUID()
        let recipeId = UUID()
        var responses = [response(changes: [change(id: id, recipeId: recipeId, title: "Soup")])]
        let store = makeStore(stack: stack, defaults: defaults) { cursor, ops in
            requests.append((cursor, ops))
            return responses.removeFirst()
        }
        await store.syncWithServer(isFollowUp: true)
        let plan = try XCTUnwrap(store.plans.first)

        try store.update(plan, date: day("2026-04-17"), mealType: .dinner, notes: "Double batch")
        responses.append(response(results: [.applied]))
        await store.syncWithServer(isFollowUp: true)

        let update = try XCTUnwrap(requests.last?.ops.first)
        XCTAssertEqual(update["op"] as? String, "update")
        XCTAssertEqual(update["fields"] as? [String: String], ["notes": "Double batch"])
        XCTAssertEqual(requests.last?.cursor, 100)
        XCTAssertEqual(store.plans.first?.notes, "Double batch")
    }

    func testADuplicateTheServerRefusesDropsTheCreateAndSaysWhy() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        // The server refuses the create and, since the plan never existed,
        // sends its id as a tombstone.
        let store = makeStore(stack: stack, defaults: defaults) { _, ops in
            let id = try XCTUnwrap((ops.first?["id"] as? String).flatMap(UUID.init(uuidString:)))
            return self.response(results: [.duplicate], deleted: [id])
        }

        // Planned on another device too, before this one synced.
        try store.add(recipeId: UUID(), recipeTitle: "Soup", thumbnailPhotoId: nil, date: day("2026-04-17"), mealType: .dinner)
        await store.syncWithServer(isFollowUp: true)

        XCTAssertTrue(store.plans.isEmpty)
        XCTAssertEqual(store.refusal, MealPlanStore.refusalMessage(.duplicate, recipeTitle: "Soup"))
    }

    func testAPlanMovedWhileItsRefusedCreateWasInFlightIsRetried() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [Request] = []
        var duringSync: (() throws -> Void)?
        let store = makeStore(stack: stack, defaults: defaults) { cursor, ops in
            requests.append((cursor, ops))
            try duringSync?()
            duringSync = nil
            let id = try XCTUnwrap((ops.first?["id"] as? String).flatMap(UUID.init(uuidString:)))
            return self.response(results: [requests.count == 1 ? .duplicate : .applied], deleted: requests.count == 1 ? [id] : [])
        }
        try store.add(recipeId: UUID(), recipeTitle: "Soup", thumbnailPhotoId: nil, date: day("2026-04-17"), mealType: .dinner)
        let original = try XCTUnwrap(store.plans.first)
        // The user moves it to lunch while the dinner create is refused.
        duringSync = { try store.update(original, date: self.day("2026-04-17"), mealType: .lunch, notes: "") }

        await store.syncWithServer(isFollowUp: true)

        let retried = try XCTUnwrap(store.plans.first)
        XCTAssertNotEqual(retried.id, original.id)
        XCTAssertEqual(retried.mealType, .lunch)
        await store.syncWithServer(isFollowUp: true)
        let retry = try XCTUnwrap(requests.last?.ops.first)
        XCTAssertEqual(retry["op"] as? String, "create")
        XCTAssertEqual((retry["fields"] as? [String: Any])?["meal_type"] as? String, "lunch")
    }

    func testDeletingARecipeTombstonesItsPlans() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let id = UUID()
        var responses = [
            response(changes: [change(id: id, recipeId: UUID(), title: "Soup")]),
            response(deleted: [id])
        ]
        let store = makeStore(stack: stack, defaults: defaults) { _, _ in responses.removeFirst() }

        await store.syncWithServer(isFollowUp: true)
        XCTAssertEqual(store.plans.map(\.id), [id])
        await store.syncWithServer(isFollowUp: true)

        XCTAssertTrue(store.plans.isEmpty)
    }

    func testADeleteGoesThroughTheServer() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [Request] = []
        let id = UUID()
        var responses = [response(changes: [change(id: id, recipeId: UUID(), title: "Soup")])]
        let store = makeStore(stack: stack, defaults: defaults) { cursor, ops in
            requests.append((cursor, ops))
            return responses.removeFirst()
        }
        await store.syncWithServer(isFollowUp: true)

        store.delete(try XCTUnwrap(store.plans.first))
        XCTAssertTrue(store.plans.isEmpty)
        responses.append(response(results: [.applied], deleted: [id]))
        await store.syncWithServer(isFollowUp: true)

        XCTAssertEqual(requests.last?.ops.first?["op"] as? String, "delete")
        XCTAssertTrue(try stack.viewContext.fetch(CachedMealPlan.fetchAll(accountKey: account)).isEmpty)
    }

    func testPlansAreIsolatedByAccount() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let store = makeStore(stack: stack, defaults: defaults) { _, _ in self.response() }

        try store.add(recipeId: UUID(), recipeTitle: "Soup", thumbnailPhotoId: nil, date: day("2026-04-17"), mealType: .dinner)
        store.setActiveAccountKey(AccountScope.key(serverURL: "https://one.test", username: "baker"))
        XCTAssertTrue(store.plans.isEmpty)
        store.setActiveAccountKey(account)

        XCTAssertEqual(store.plans.map(\.recipeTitle), ["Soup"])
    }

    func testReconnectingSendsEditsMadeOffline() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requests: [Request] = []
        let store = makeStore(stack: stack, defaults: defaults) { cursor, ops in
            requests.append((cursor, ops))
            return self.response(results: ops.map { _ in .applied })
        }
        try store.add(recipeId: UUID(), recipeTitle: "Soup", thumbnailPhotoId: nil, date: day("2026-04-17"), mealType: .dinner)

        await store.syncIfPending()

        XCTAssertEqual(requests.count, 1)
        XCTAssertEqual(requests.first?.ops.first?["op"] as? String, "create")
        XCTAssertTrue(try stack.viewContext.fetch(CachedMealPlan.fetchPendingSync(accountKey: account)).isEmpty)
    }

    func testReconnectingDuringAFailingSyncRetriesOnceItEnds() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var store: MealPlanStore!
        var requestCount = 0
        store = makeStore(stack: stack, defaults: defaults) { _, ops in
            requestCount += 1
            if requestCount == 1 {
                await store.syncIfPending()
                throw URLError(.notConnectedToInternet)
            }
            return self.response(results: ops.map { _ in .applied })
        }
        try store.add(recipeId: UUID(), recipeTitle: "Soup", thumbnailPhotoId: nil, date: day("2026-04-17"), mealType: .dinner)

        await store.syncWithServer()

        XCTAssertEqual(requestCount, 2)
        XCTAssertTrue(try stack.viewContext.fetch(CachedMealPlan.fetchPendingSync(accountKey: account)).isEmpty)
    }

    func testReconnectingWithNothingPendingDoesNotSync() async throws {
        let (stack, defaults) = makeStorage()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        var requestCount = 0
        let store = makeStore(stack: stack, defaults: defaults) { _, _ in
            requestCount += 1
            return self.response()
        }

        await store.syncIfPending()

        XCTAssertEqual(requestCount, 0)
    }

    private func makeStore(
        stack: CoreDataStack,
        defaults: UserDefaults,
        sync: @escaping (Int64?, [[String: Any]]) async throws -> SyncMealPlansResponse
    ) -> MealPlanStore {
        MealPlanStore(
            coreDataStack: stack,
            userDefaults: defaults,
            initialAccountKey: account,
            automaticallySync: false,
            syncMealPlans: sync
        )
    }

    private func response(
        results: [OpResult] = [],
        changes: [SyncMealPlan] = [],
        deleted: [UUID] = []
    ) -> SyncMealPlansResponse {
        SyncMealPlansResponse(changes: changes, cursor: 100, deleted: deleted, fullResync: false, results: results)
    }

    private func change(id: UUID, recipeId: UUID, title: String) -> SyncMealPlan {
        SyncMealPlan(
            changeSeq: 1,
            id: id,
            mealDate: MealPlanDateSupport.apiDate(fromDay: "2026-04-17")!,
            mealType: .dinner,
            recipeId: recipeId,
            recipeTitle: title
        )
    }

    private func day(_ string: String) -> Date {
        SharedDateFormatters.localDateOnly.date(from: string)!
    }

    private func makeStorage() -> (CoreDataStack, UserDefaults) {
        let defaults = UserDefaults(suiteName: defaultsSuiteName)!
        defaults.removePersistentDomain(forName: defaultsSuiteName)
        return (CoreDataTestStack.makeStack(), defaults)
    }

    private var defaultsSuiteName: String { "MealPlanStoreTests.\(name)" }
    private var account: String { AccountScope.key(serverURL: "https://one.test", username: "chef") }
}
