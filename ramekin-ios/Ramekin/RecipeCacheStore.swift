import CoreData
import Foundation

struct CachedRecipeSearchDocument {
    let summary: RecipeSummary
    let ingredients: [Ingredient]
    /// The server-produced JSONB-to-text rendering of the ingredients — the
    /// exact haystack server search matches bare text against. Distinct from
    /// `ingredients`, which the relevance scorer flattens itself.
    let ingredientMatchText: String
    let instructions: String
    let notes: String?
}

/// Durable mid-sweep progress for a paged sync, so an interrupted sweep
/// resumes where it left off instead of re-fetching every page.
struct PendingSyncSweep: Codable, Equatable {
    /// The cursor every page of the sweep filters on. Nil for a full sync.
    let since: Int64?
    /// Last recipe ID applied; the next page starts past it.
    let afterId: UUID
    /// The sweep's first-page watermark — what gets persisted as the sync
    /// cursor once the sweep completes. Later pages' watermarks are too high:
    /// they would skip changes that committed mid-sweep in id ranges the
    /// sweep had already passed.
    let watermark: Int64
}

@MainActor
final class RecipeCacheStore {
    static let shared = RecipeCacheStore()
    // v4 added ingredientMatchText; bumping forces a full re-sync so every
    // cached recipe carries it.
    private static let cacheSchemaVersion = 4

    private let coreDataStack: CoreDataStack
    private let userDefaults: UserDefaults

    init(coreDataStack: CoreDataStack = .shared, userDefaults: UserDefaults = .standard) {
        self.coreDataStack = coreDataStack
        self.userDefaults = userDefaults
    }

    func currentAccountKey() -> String? {
        AccountScope.currentAccountKey()
    }

    func syncCursor(accountKey: String) -> Int64? {
        userDefaults.object(forKey: syncCursorKey(accountKey: accountKey)) as? Int64
    }

    func setSyncCursor(_ cursor: Int64, accountKey: String) {
        userDefaults.set(cursor, forKey: syncCursorKey(accountKey: accountKey))
    }

    func clearSyncCursor(accountKey: String) {
        userDefaults.removeObject(forKey: syncCursorKey(accountKey: accountKey))
        clearPendingSyncSweep(accountKey: accountKey)
    }

    func pendingSyncSweep(accountKey: String) -> PendingSyncSweep? {
        guard let data = userDefaults.data(forKey: pendingSweepKey(accountKey: accountKey)) else {
            return nil
        }
        do {
            return try JSONDecoder().decode(PendingSyncSweep.self, from: data)
        } catch {
            fatalError("Pending sync sweep is invalid JSON: \(error)")
        }
    }

    func setPendingSyncSweep(_ sweep: PendingSyncSweep, accountKey: String) {
        do {
            let data = try JSONEncoder().encode(sweep)
            userDefaults.set(data, forKey: pendingSweepKey(accountKey: accountKey))
        } catch {
            fatalError("Failed to encode pending sync sweep: \(error)")
        }
    }

    func clearPendingSyncSweep(accountKey: String) {
        userDefaults.removeObject(forKey: pendingSweepKey(accountKey: accountKey))
    }

    func loadSearchDocuments(accountKey: String) throws -> [CachedRecipeSearchDocument] {
        try purgeRowsWrittenByOlderSchema(accountKey: accountKey)
        let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
        request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
        request.sortDescriptors = [
            NSSortDescriptor(keyPath: \CachedRecipe.updatedAt, ascending: false),
            NSSortDescriptor(keyPath: \CachedRecipe.id, ascending: true)
        ]
        return try coreDataStack.viewContext.fetch(request).map(searchDocument)
    }

    /// Rows written under an older cache schema must never be served: Core
    /// Data's lightweight migration backfills columns the old schema lacked
    /// with defaults (e.g. an empty ingredient match text), so searching them
    /// would silently omit recipes the server would return. The schema bump
    /// already forces a full re-sync; this drops the migrated rows so the
    /// window before that sync completes serves nothing instead of wrong
    /// results.
    private func purgeRowsWrittenByOlderSchema(accountKey: String) throws {
        let key = rowsSchemaVersionKey(accountKey: accountKey)
        guard userDefaults.integer(forKey: key) != Self.cacheSchemaVersion else {
            return
        }
        let context = coreDataStack.viewContext
        let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
        request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
        let staleRows = try context.fetch(request)
        guard !staleRows.isEmpty else {
            return
        }
        for row in staleRows {
            context.delete(row)
        }
        try coreDataStack.saveContextOrThrow()
    }

    func apply(syncResponse: SyncRecipesResponse, accountKey: String) throws {
        let context = coreDataStack.viewContext

        for id in syncResponse.deleted {
            let request = fetchRequest(accountKey: accountKey, id: id)
            if let cachedRecipe = try context.fetch(request).first {
                context.delete(cachedRecipe)
            }
        }

        for recipe in syncResponse.recipes {
            let cachedRecipe = try context.fetch(fetchRequest(accountKey: accountKey, id: recipe.id)).first
                ?? CachedRecipe(context: context)
            cachedRecipe.accountKey = accountKey
            cachedRecipe.id = recipe.id
            cachedRecipe.ingredientMatchText = recipe.ingredientMatchText
            cachedRecipe.ingredientsJSON = try ingredientsJSON(recipe.ingredients)
            cachedRecipe.instructions = recipe.instructions
            cachedRecipe.notes = recipe.notes
            cachedRecipe.title = recipe.title
            cachedRecipe.summaryDescription = recipe.description
            cachedRecipe.tagsJSON = try tagsJSON(recipe.tags)
            cachedRecipe.thumbnailPhotoId = recipe.thumbnailPhotoId
            cachedRecipe.rating = recipe.rating.map(String.init)
            cachedRecipe.createdAt = recipe.createdAt
            cachedRecipe.updatedAt = recipe.updatedAt
        }

        try coreDataStack.saveContextOrThrow()
        userDefaults.set(Self.cacheSchemaVersion, forKey: rowsSchemaVersionKey(accountKey: accountKey))
    }

    private func fetchRequest(accountKey: String, id: UUID) -> NSFetchRequest<CachedRecipe> {
        let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
        request.predicate = NSPredicate(format: "accountKey == %@ AND id == %@", accountKey, id as CVarArg)
        request.fetchLimit = 1
        return request
    }

    private func searchDocument(from cachedRecipe: CachedRecipe) -> CachedRecipeSearchDocument {
        guard let createdAt = cachedRecipe.createdAt,
              let id = cachedRecipe.id,
              let ingredientMatchText = cachedRecipe.ingredientMatchText,
              let ingredientsJSON = cachedRecipe.ingredientsJSON,
              let instructions = cachedRecipe.instructions,
              let tagsJSON = cachedRecipe.tagsJSON,
              let title = cachedRecipe.title,
              let updatedAt = cachedRecipe.updatedAt
        else {
            fatalError("CachedRecipe is missing required fields")
        }

        let summary = RecipeSummary(
            createdAt: createdAt,
            description: cachedRecipe.summaryDescription,
            id: id,
            rating: cachedRecipe.rating.flatMap(Int.init),
            tags: tags(from: tagsJSON),
            thumbnailPhotoId: cachedRecipe.thumbnailPhotoId,
            title: title,
            updatedAt: updatedAt
        )
        return CachedRecipeSearchDocument(
            summary: summary,
            ingredients: ingredients(from: ingredientsJSON),
            ingredientMatchText: ingredientMatchText,
            instructions: instructions,
            notes: cachedRecipe.notes
        )
    }

    private func ingredientsJSON(_ ingredients: [Ingredient]) throws -> String {
        let data = try JSONEncoder().encode(ingredients)
        guard let json = String(data: data, encoding: .utf8) else {
            fatalError("Failed to encode cached recipe ingredients as UTF-8")
        }
        return json
    }

    private func ingredients(from json: String) -> [Ingredient] {
        guard let data = json.data(using: .utf8) else {
            fatalError("Cached recipe ingredients are not UTF-8")
        }
        do {
            return try JSONDecoder().decode([Ingredient].self, from: data)
        } catch {
            fatalError("Cached recipe ingredients are invalid JSON: \(error)")
        }
    }

    private func tagsJSON(_ tags: [String]) throws -> String {
        let data = try JSONEncoder().encode(tags)
        guard let json = String(data: data, encoding: .utf8) else {
            fatalError("Failed to encode cached recipe tags as UTF-8")
        }
        return json
    }

    private func tags(from json: String) -> [String] {
        guard let data = json.data(using: .utf8) else {
            fatalError("Cached recipe tags are not UTF-8")
        }
        do {
            return try JSONDecoder().decode([String].self, from: data)
        } catch {
            fatalError("Cached recipe tags are invalid JSON: \(error)")
        }
    }

    private func syncCursorKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(
            prefix: "recipe_cache_v\(Self.cacheSchemaVersion)_sync_cursor",
            accountKey: accountKey
        )
    }

    private func pendingSweepKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(
            prefix: "recipe_cache_v\(Self.cacheSchemaVersion)_pending_sweep",
            accountKey: accountKey
        )
    }

    /// Deliberately unversioned, unlike the keys above: it records which
    /// schema version last wrote rows, so it must survive a version bump for
    /// the purge check to see the old value.
    private func rowsSchemaVersionKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(
            prefix: "recipe_cache_rows_schema_version",
            accountKey: accountKey
        )
    }
}
