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
            instructions: "Mix and bake.",
            notes: "Cool completely."
        )
        let cursor: Int64 = 300

        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: cursor,
                deleted: [],
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
        XCTAssertEqual(document.instructions, "Mix and bake.")
        XCTAssertEqual(document.notes, "Cool completely.")
        XCTAssertEqual(store.syncCursor(accountKey: accountKey), cursor)
    }

    func testApplyReplacesSearchableFieldsOnUpdate() throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let id = UUID()
        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 300,
                deleted: [],
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
                recipes: [recipe]
            ),
            accountKey: accountKey
        )

        try store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 400,
                deleted: [recipe.id],
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
        instructions: String = "Cook it.",
        notes: String? = nil,
        updatedAt: Date = Date(timeIntervalSince1970: 200)
    ) -> SyncRecipe {
        SyncRecipe(
            createdAt: Date(timeIntervalSince1970: 100),
            description: "A cached recipe",
            id: id,
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
