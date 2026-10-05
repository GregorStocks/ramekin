import CoreData
import XCTest
@testable import Ramekin

/// Corrupt cache state must be dropped and re-synced, not crash the app on
/// every launch.
@MainActor
final class RecipeCacheStoreRecoveryTests: XCTestCase {
    func testInvalidPendingSyncSweepIsDiscardedInsteadOfCrashing() throws {
        let (store, defaults) = makeStore()
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        store.setPendingSyncSweep(
            PendingSyncSweep(since: nil, afterId: UUID(), watermark: 250),
            accountKey: accountKey
        )
        let sweepKey = try XCTUnwrap(defaults.dictionaryRepresentation().keys.first {
            $0.contains("pending_sweep")
        })
        defaults.set(Data("not json".utf8), forKey: sweepKey)

        XCTAssertNil(store.pendingSyncSweep(accountKey: accountKey))
        XCTAssertNil(defaults.object(forKey: sweepKey))
    }

    func testCorruptCachedRowsAreDroppedAndForceFullSync() async throws {
        let stack = CoreDataTestStack.makeStack()
        let (store, defaults) = makeStore(coreDataStack: stack)
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        let good = makeRecipe()
        // Stamps the current schema marker so the rows below aren't purged
        // as older-schema rows before they're decoded.
        try await store.apply(
            syncResponse: SyncRecipesResponse(
                cursor: 300,
                deleted: [],
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion,
                recipes: [good]
            ),
            accountKey: accountKey
        )
        store.setSyncCursor(300, accountKey: accountKey)
        store.setPendingSyncSweep(
            PendingSyncSweep(since: 300, afterId: UUID(), watermark: 400),
            accountKey: accountKey
        )

        // Written behind the store's back: rows whose ingredients or tags
        // JSON no longer decodes.
        let context = stack.newBackgroundContext()
        try await context.perform { [accountKey] in
            for (title, ingredientsJSON, tagsJSON) in [
                ("Bad ingredients", "not json", "[]"),
                ("Bad tags", "[]", "{")
            ] {
                let row = CachedRecipe(context: context)
                row.accountKey = accountKey
                row.id = UUID()
                row.title = title
                row.ingredientMatchText = "[]"
                row.ingredientsJSON = ingredientsJSON
                row.tagsJSON = tagsJSON
                row.instructions = "Cook it."
                row.createdAt = Date(timeIntervalSince1970: 100)
                row.updatedAt = Date(timeIntervalSince1970: 200)
            }
            try context.save()
        }

        let documents = try await store.loadSearchDocuments(accountKey: accountKey)

        XCTAssertEqual(documents.map(\.summary.id), [good.id])
        let remainingRows = try await context.perform { [accountKey] in
            let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
            request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
            return try context.count(for: request)
        }
        XCTAssertEqual(remainingRows, 1)
        // The next sync is a full one, which rewrites the dropped recipes.
        XCTAssertNil(store.syncCursor(accountKey: accountKey))
        XCTAssertNil(store.pendingSyncSweep(accountKey: accountKey))
    }

    private var accountKey: String { "https://example.test|chef" }
    private var defaultsSuiteName: String { "RecipeCacheStoreRecoveryTests" }

    private func makeStore(
        coreDataStack: CoreDataStack = CoreDataTestStack.makeStack()
    ) -> (RecipeCacheStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: defaultsSuiteName)!
        defaults.removePersistentDomain(forName: defaultsSuiteName)
        return (RecipeCacheStore(coreDataStack: coreDataStack, userDefaults: defaults), defaults)
    }

    private func makeRecipe() -> SyncRecipe {
        SyncRecipe(
            createdAt: Date(timeIntervalSince1970: 100),
            description: nil,
            id: UUID(),
            ingredientMatchText: "[]",
            ingredients: [],
            instructions: "Cook it.",
            notes: nil,
            rating: nil,
            tags: ["Dinner"],
            thumbnailPhotoId: nil,
            title: "Good Recipe",
            updatedAt: Date(timeIntervalSince1970: 200)
        )
    }
}
