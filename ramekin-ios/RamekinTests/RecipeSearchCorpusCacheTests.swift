import XCTest
@testable import Ramekin

@MainActor
final class RecipeSearchCorpusCacheTests: XCTestCase {
    private let accountKey = "https://example.test|chef"
    private let defaultsSuiteName = "RecipeSearchCorpusCacheTests"

    func testSearchCorpusIsReusedUntilTheCacheChanges() async throws {
        let defaults = UserDefaults(suiteName: defaultsSuiteName)!
        defaults.removePersistentDomain(forName: defaultsSuiteName)
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let store = RecipeCacheStore(coreDataStack: CoreDataTestStack.makeStack(), userDefaults: defaults)
        let recipeId = UUID()
        try await apply([makeRecipe(id: recipeId, instructions: "Whisk the eggs.")], to: store)

        let first = try await store.loadSearchCorpus(accountKey: accountKey)
        XCTAssertEqual(first.entries.map(\.summary.id), [recipeId])

        // A sync page that changes nothing keeps the prepared corpus.
        try await apply([], to: store)
        let unchanged = try await store.loadSearchCorpus(accountKey: accountKey)
        XCTAssertEqual(unchanged.version, first.version)

        // An edit has to show up in the next search.
        try await apply([makeRecipe(id: recipeId, instructions: "Fold in the flour.")], to: store)
        let updated = try await store.loadSearchCorpus(accountKey: accountKey)
        XCTAssertNotEqual(updated.version, first.version)
        let parsed = RecipeSearchSupport.parse("flour")
        XCTAssertEqual(
            RecipeSearchSupport.execute(corpus: updated, parsed: parsed, sortBy: nil, sortDir: nil).map(\.id),
            [recipeId]
        )
        XCTAssertTrue(RecipeSearchSupport.execute(corpus: first, parsed: parsed, sortBy: nil, sortDir: nil).isEmpty)

        // Another account never gets this account's corpus.
        let other = try await store.loadSearchCorpus(accountKey: "https://example.test|other")
        XCTAssertTrue(other.isEmpty)
    }

    private func apply(_ changes: [SyncRecipe], to store: RecipeCacheStore) async throws {
        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: changes,
                cursor: 300,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            accountKey: accountKey
        )
    }

    private func makeRecipe(id: UUID, instructions: String) -> SyncRecipe {
        SyncRecipe(
            createdAt: Date(timeIntervalSince1970: 100),
            description: nil,
            id: id,
            ingredientMatchText: "[]",
            ingredients: [],
            instructions: instructions,
            notes: nil,
            photos: [],
            rating: nil,
            tags: [],
            thumbnailPhotoId: nil,
            title: "Cached Recipe",
            updatedAt: Date(timeIntervalSince1970: 200)
        )
    }
}
