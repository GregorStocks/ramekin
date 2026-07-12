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
        // the xid cursor, so the store must ignore it and take a full sync.
        defaults.set(
            Date(timeIntervalSince1970: 100),
            forKey: "recipe_cache_v2_last_sync_at_\(encodedAccountKey)"
        )

        XCTAssertNil(store.syncCursor(accountKey: accountKey))
    }

    func testRowsWrittenByOlderSchemaArePurgedNotServed() throws {
        // Simulate an upgrade: rows exist, but the marker recording which
        // schema wrote them predates the current version (or is absent, as
        // for any pre-v4 install). Core Data migration backfills new columns
        // with defaults, so serving such rows would under-match searches.
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 300,
                deleted: [],
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                recipes: [makeRecipe()]
            ),
            accountKey: accountKey
        )
        let markerKey = defaults.dictionaryRepresentation().keys.first {
            $0.hasPrefix("recipe_cache_rows_schema_version")
        }
        defaults.removeObject(forKey: try XCTUnwrap(markerKey))

        XCTAssertTrue(try store.loadSearchDocuments(accountKey: accountKey).isEmpty)

        // A fresh apply stamps the current schema version, so its rows serve.
        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 400,
                deleted: [],
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                recipes: [makeRecipe()]
            ),
            accountKey: accountKey
        )
        XCTAssertEqual(try store.loadSearchDocuments(accountKey: accountKey).count, 1)
    }

    func testApplyPopulatesSearchableRecipeBody() throws {
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

        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 300,
                deleted: [],
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                recipes: [recipe]
            ),
            accountKey: accountKey
        )

        let documents = try store.loadSearchDocuments(accountKey: accountKey)
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

    func testApplyReplacesSearchableFieldsOnUpdate() throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let id = UUID()
        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 300,
                deleted: [],
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                recipes: [makeRecipe(id: id, instructions: "Old instructions", notes: "Old notes")]
            ),
            accountKey: accountKey
        )
        let updated = makeRecipe(
            id: id,
            ingredients: [Ingredient(item: "rice", measurements: [], note: nil, section: nil)],
            instructions: "New instructions",
            notes: nil,
            updatedAt: Date(timeIntervalSince1970: 400)
        )

        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 500,
                deleted: [],
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                recipes: [updated]
            ),
            accountKey: accountKey
        )

        let documents = try store.loadSearchDocuments(accountKey: accountKey)
        XCTAssertEqual(documents.count, 1)
        XCTAssertEqual(documents[0].ingredients, updated.ingredients)
        XCTAssertEqual(documents[0].instructions, "New instructions")
        XCTAssertNil(documents[0].notes)
    }

    func testApplyRemovesDeletedRecipe() throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let recipe = makeRecipe()
        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 300,
                deleted: [],
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                recipes: [recipe]
            ),
            accountKey: accountKey
        )

        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 400,
                deleted: [recipe.id],
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                recipes: []
            ),
            accountKey: accountKey
        )

        XCTAssertTrue(try store.loadSearchDocuments(accountKey: accountKey).isEmpty)
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
