import CoreData
import XCTest
@testable import Ramekin

@MainActor
final class RecipeCacheStoreTests: XCTestCase {
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
        let syncTimestamp = Date(timeIntervalSince1970: 300)

        try store.apply(
            syncResponse: SyncRecipesResponse(
                deleted: [],
                recipes: [recipe],
                syncTimestamp: syncTimestamp
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
        XCTAssertEqual(store.lastSyncAt(accountKey: accountKey), syncTimestamp)
    }

    func testApplyReplacesSearchableFieldsOnUpdate() throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let id = UUID()
        try store.apply(
            syncResponse: SyncRecipesResponse(
                deleted: [],
                recipes: [makeRecipe(id: id, instructions: "Old instructions", notes: "Old notes")],
                syncTimestamp: Date(timeIntervalSince1970: 300)
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
                deleted: [],
                recipes: [updated],
                syncTimestamp: Date(timeIntervalSince1970: 500)
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
                deleted: [],
                recipes: [recipe],
                syncTimestamp: Date(timeIntervalSince1970: 300)
            ),
            accountKey: accountKey
        )

        try store.apply(
            syncResponse: SyncRecipesResponse(
                deleted: [recipe.id],
                recipes: [],
                syncTimestamp: Date(timeIntervalSince1970: 400)
            ),
            accountKey: accountKey
        )

        XCTAssertTrue(try store.loadSearchDocuments(accountKey: accountKey).isEmpty)
    }

    private var accountKey: String { "https://example.test|chef" }
    private var defaultsSuiteName: String { "RecipeCacheStoreTests" }

    private func makeStore() -> (RecipeCacheStore, UserDefaults) {
        let container = NSPersistentContainer(name: "Ramekin")
        let description = NSPersistentStoreDescription()
        description.type = NSInMemoryStoreType
        container.persistentStoreDescriptions = [description]
        container.loadPersistentStores { _, error in
            XCTAssertNil(error)
        }
        let defaults = UserDefaults(suiteName: defaultsSuiteName)!
        defaults.removePersistentDomain(forName: defaultsSuiteName)
        return (
            RecipeCacheStore(coreDataStack: CoreDataStack(container: container), userDefaults: defaults),
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
