import CoreData
import XCTest
@testable import Ramekin

@MainActor
final class RecipeCacheStoreTests: XCTestCase {
    func testCacheSchemaChangeForcesFullSync() {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let encodedAccountKey = Data(accountKey.utf8).base64EncodedString()
        // The timestamp cursor a pre-v3 install persisted. It is meaningless to
        // the change cursor, so the store must ignore it and take a full sync.
        defaults.set(
            Date(timeIntervalSince1970: 100),
            forKey: "recipe_cache_v2_last_sync_at_\(encodedAccountKey)"
        )

        XCTAssertNil(store.syncCursor(accountKey: accountKey))
    }

    func testRowsWrittenByOlderSchemaArePurgedNotServed() async throws {
        // Simulate an upgrade: rows exist, but the marker recording which
        // schema wrote them predates the current version (or is absent, as
        // for any pre-v4 install). Core Data migration backfills new columns
        // with defaults, so serving such rows would under-match searches.
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [makeRecipe()],
                cursor: 300,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )
        let markerKey = defaults.dictionaryRepresentation().keys.first {
            $0.hasPrefix("recipe_cache_rows_schema_version")
        }
        defaults.removeObject(forKey: try XCTUnwrap(markerKey))

        let remaining = try await store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertTrue(remaining.isEmpty)

        // A fresh apply stamps the current schema version, so its rows serve.
        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [makeRecipe()],
                cursor: 400,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )
        let served = try await store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertEqual(served.count, 1)
    }

    func testApplyPopulatesSearchableRecipeBody() async throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let recipe = makeRecipe(
            ingredients: [
                Ingredient(
                    item: "flour",
                    measurements: [Measurement(amount: "2", unit: "cups")],
                    note: "sifted",
                    section: "Cake"
                )
            ],
            ingredientMatchText: "[{\"item\": \"flour\"}]",
            instructions: "Mix and bake.",
            notes: "Cool completely."
        )

        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [recipe],
                cursor: 300,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )

        let documents = try await store.loadSearchDocuments(accountKey: accountKey)
        let document = try XCTUnwrap(documents.first)
        XCTAssertEqual(documents.count, 1)
        XCTAssertEqual(document.summary.id, recipe.id)
        XCTAssertEqual(document.summary.title, recipe.title)
        XCTAssertEqual(document.ingredients, recipe.ingredients)
        XCTAssertEqual(document.ingredientMatchText, "[{\"item\": \"flour\"}]")
        XCTAssertEqual(document.instructions, "Mix and bake.")
        XCTAssertEqual(document.notes, "Cool completely.")
        // The cursor only advances when a sweep completes, never per page.
        XCTAssertNil(store.syncCursor(accountKey: accountKey))
    }

    func testPendingSyncSweepRoundTripsAndClearsWithCursor() {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let sweep = PendingSyncSweep(since: nil, afterId: UUID(), watermark: 250)

        store.setPendingSyncSweep(sweep, accountKey: accountKey)
        store.setSyncCursor(300, accountKey: accountKey)

        XCTAssertEqual(store.pendingSyncSweep(accountKey: accountKey), sweep)
        XCTAssertEqual(store.syncCursor(accountKey: accountKey), 300)
        XCTAssertNil(store.pendingSyncSweep(accountKey: "https://example.test|other"))

        store.clearSyncCursor(accountKey: accountKey)

        XCTAssertNil(store.syncCursor(accountKey: accountKey))
        // Invalidating the cache restarts the sweep from scratch: resuming a
        // pending sweep against a different `since` would skip pages.
        XCTAssertNil(store.pendingSyncSweep(accountKey: accountKey))
    }

    func testApplyReplacesSearchableFieldsOnUpdate() async throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let id = UUID()
        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [makeRecipe(id: id, instructions: "Old instructions", notes: "Old notes")],
                cursor: 300,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )
        let updated = makeRecipe(
            id: id,
            ingredients: [Ingredient(item: "rice", measurements: [], note: nil, section: nil)],
            instructions: "New instructions",
            notes: nil,
            updatedAt: Date(timeIntervalSince1970: 400)
        )

        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [updated],
                cursor: 500,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )

        let documents = try await store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertEqual(documents.count, 1)
        XCTAssertEqual(documents[0].ingredients, updated.ingredients)
        XCTAssertEqual(documents[0].instructions, "New instructions")
        XCTAssertNil(documents[0].notes)
    }

    func testApplyRemovesDeletedRecipe() async throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let recipe = makeRecipe()
        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [recipe],
                cursor: 300,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )

        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [],
                cursor: 400,
                deleted: [recipe.id],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )

        let remaining = try await store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertTrue(remaining.isEmpty)
    }

    func testApplyUpdatesInsertsAndDeletesInOnePage() async throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let kept = makeRecipe(instructions: "Old instructions")
        let deleted = makeRecipe()
        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [kept, deleted],
                cursor: 300,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )
        let added = makeRecipe(updatedAt: Date(timeIntervalSince1970: 100))

        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [makeRecipe(id: kept.id, instructions: "New instructions"), added],
                cursor: 400,
                deleted: [deleted.id, UUID()],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )

        let documents = try await store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertEqual(documents.map(\.summary.id), [kept.id, added.id])
        XCTAssertEqual(documents[0].instructions, "New instructions")
    }

    func testApplyLeavesOtherAccountsRowsAlone() async throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let otherAccountKey = "https://example.test|other"
        let recipe = makeRecipe()
        for key in [accountKey, otherAccountKey] {
            try await store.apply(
                syncResponse: SyncRecipesResponse(
                    changes: [recipe],
                    cursor: 300,
                    deleted: [],
                    fullResync: false,
                    hasMore: false,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion
                ),
                afterId: nil,
                accountKey: key
            )
        }

        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: [],
                cursor: 400,
                deleted: [recipe.id],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )

        let mine = try await store.loadSearchDocuments(accountKey: accountKey)
        let theirs = try await store.loadSearchDocuments(accountKey: otherAccountKey)
        XCTAssertTrue(mine.isEmpty)
        XCTAssertEqual(theirs.map(\.summary.id), [recipe.id])
    }

    func testStoreWorksOffTheMainActor() async throws {
        // Applying a full sync on the main thread stalled the app for tens of
        // seconds on CI; the store must not be bound to the main actor.
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let recipe = makeRecipe()
        let accountKey = accountKey

        let documents = try await Task.detached {
            XCTAssertFalse(Thread.isMainThread)
            try await store.apply(
                syncResponse: SyncRecipesResponse(
                    changes: [recipe],
                    cursor: 300,
                    deleted: [],
                    fullResync: false,
                    hasMore: false,
                    normalizationContractVersion: SearchNormalizationSupport.contractVersion
                ),
                afterId: nil,
                accountKey: accountKey
            )
            return try await store.loadSearchDocuments(accountKey: accountKey)
        }.value

        XCTAssertEqual(documents.map(\.summary.id), [recipe.id])
    }

    private var accountKey: String { "https://example.test|chef" }
    private var defaultsSuiteName: String { "RecipeCacheStoreTests" }

    private func makeStore() -> (RecipeCacheStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: defaultsSuiteName)!
        defaults.removePersistentDomain(forName: defaultsSuiteName)
        return (
            RecipeCacheStore(coreDataStack: CoreDataTestStack.makeStack(), userDefaults: defaults),
            defaults
        )
    }

    private func makeRecipe(
        id: UUID = UUID(),
        ingredients: [Ingredient] = [],
        ingredientMatchText: String = "[]",
        instructions: String = "Cook it.",
        notes: String? = nil,
        updatedAt: Date = Date(timeIntervalSince1970: 200)
    ) -> SyncRecipe {
        SyncRecipe(
            createdAt: Date(timeIntervalSince1970: 100),
            description: "A cached recipe",
            id: id,
            ingredientMatchText: ingredientMatchText,
            ingredients: ingredients,
            instructions: instructions,
            notes: notes,
            rating: 5,
            tags: ["Dinner"],
            thumbnailPhotoId: UUID(),
            title: "Cached Recipe",
            updatedAt: updatedAt
        )
    }
}

// MARK: - Full resync

extension RecipeCacheStoreTests {
    func testFullResyncDropsCachedRecipesTheServerNoLongerHas() async throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let kept = makeRecipe()
        let lost = makeRecipe()
        try await store.apply(syncResponse: page([kept, lost]), afterId: nil, accountKey: accountKey)

        // The server was restored from a backup taken before `lost` existed:
        // it has neither the recipe nor a tombstone for it.
        try await store.apply(
            syncResponse: page([kept], fullResync: true),
            afterId: nil,
            accountKey: accountKey
        )

        let documents = try await store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertEqual(documents.map(\.summary.id), [kept.id])
    }

    func testFullResyncPagePrunesOnlyItsOwnIdRange() async throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let ids = ["1", "3", "5", "7"].map(Self.orderedId)
        try await store.apply(
            syncResponse: page(ids.map { makeRecipe(id: $0) }),
            afterId: nil,
            accountKey: accountKey
        )

        // The server only has ids[0] and ids[2]. The first page settles IDs
        // through ids[0], so nothing past it may be dropped yet.
        try await store.apply(
            syncResponse: page([makeRecipe(id: ids[0])], fullResync: true, hasMore: true),
            afterId: nil,
            accountKey: accountKey
        )
        var cached = try await store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertEqual(Set(cached.map(\.summary.id)), Set(ids))

        try await store.apply(
            syncResponse: page([makeRecipe(id: ids[2])], fullResync: true),
            afterId: ids[0],
            accountKey: accountKey
        )
        cached = try await store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertEqual(Set(cached.map(\.summary.id)), [ids[0], ids[2]])
    }

    func testExistingRecipeIdsReportsOnlyThisAccountsCachedRecipes() async throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let mine = makeRecipe()
        let theirs = makeRecipe()
        let missing = UUID()
        let ids = [mine.id, theirs.id, missing]

        try await store.apply(syncResponse: page([mine]), afterId: nil, accountKey: accountKey)
        try await store.apply(syncResponse: page([theirs]), afterId: nil, accountKey: "https://example.test|other")

        // Until a sweep completes, a missing recipe may just be unsynced.
        let midSweep = try await store.existingRecipeIds(ids, accountKey: accountKey)
        XCTAssertNil(midSweep)

        store.setSyncCursor(300, accountKey: accountKey)

        let existing = try await store.existingRecipeIds(ids, accountKey: accountKey)
        XCTAssertEqual(existing, [mine.id])
    }

    func testFullResyncPageWithMoreButNoRecipesSettlesNothing() {
        let response = page([], fullResync: true, hasMore: true)

        XCTAssertFalse(RecipeSyncSweep.fullResyncPageCovers(Self.orderedId("9"), afterId: nil, response: response))
    }

    func testFullResyncPageCoversIdsInServerTextOrder() {
        // The server orders lowercase hex text, where digits sort before letters.
        let response = page([makeRecipe(id: Self.orderedId("a"))], fullResync: true, hasMore: true)

        XCTAssertTrue(RecipeSyncSweep.fullResyncPageCovers(Self.orderedId("9"), afterId: nil, response: response))
        XCTAssertFalse(RecipeSyncSweep.fullResyncPageCovers(Self.orderedId("b"), afterId: nil, response: response))
        XCTAssertFalse(
            RecipeSyncSweep.fullResyncPageCovers(Self.orderedId("9"), afterId: Self.orderedId("9"), response: response)
        )
    }

    /// An ID whose position in server order is set by its first hex digit.
    private static func orderedId(_ firstDigit: String) -> UUID {
        UUID(uuidString: "\(firstDigit)0000000-0000-0000-0000-000000000000")!
    }

    private func page(
        _ recipes: [SyncRecipe],
        fullResync: Bool = false,
        hasMore: Bool = false
    ) -> SyncRecipesResponse {
        SyncRecipesResponse(
            changes: recipes,
            cursor: 300,
            deleted: [],
            fullResync: fullResync,
            hasMore: hasMore,
            normalizationContractVersion: SearchNormalizationSupport.contractVersion
        )
    }
}
