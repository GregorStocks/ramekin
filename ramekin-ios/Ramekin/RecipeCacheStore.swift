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

enum RecipeCacheError: LocalizedError {
    case unencodableField(String)

    var errorDescription: String? {
        switch self {
        case let .unencodableField(field):
            return "Could not encode recipe \(field) for the offline cache"
        }
    }
}

/// Core Data work runs on a private background context: applying a full
/// sync and decoding the whole cache on the main thread stalled the UI for
/// tens of seconds on slow devices.
final class RecipeCacheStore {
    static let shared = RecipeCacheStore()
    // v4 added ingredientMatchText; bumping forces a full re-sync so every
    // cached recipe carries it.
    private static let cacheSchemaVersion = 4

    /// Only touched inside `perform`.
    private let backgroundContext: NSManagedObjectContext
    private let userDefaults: UserDefaults
    private let logger = DebugLogger.shared

    init(coreDataStack: CoreDataStack = .shared, userDefaults: UserDefaults = .standard) {
        backgroundContext = coreDataStack.newBackgroundContext()
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
            // Dropping it only costs re-fetching the pages it had covered.
            logger.log("Discarding invalid pending sync sweep: \(error)", source: "RecipeCache")
            clearPendingSyncSweep(accountKey: accountKey)
            return nil
        }
    }

    func setPendingSyncSweep(_ sweep: PendingSyncSweep, accountKey: String) {
        do {
            let data = try JSONEncoder().encode(sweep)
            userDefaults.set(data, forKey: pendingSweepKey(accountKey: accountKey))
        } catch {
            // Without it an interrupted sweep restarts from its first page.
            logger.log("Failed to encode pending sync sweep: \(error)", source: "RecipeCache")
        }
    }

    func clearPendingSyncSweep(accountKey: String) {
        userDefaults.removeObject(forKey: pendingSweepKey(accountKey: accountKey))
    }

    func loadSearchDocuments(accountKey: String) async throws -> [CachedRecipeSearchDocument] {
        try await backgroundContext.perform { [self] in
            try purgeRowsWrittenByOlderSchema(accountKey: accountKey)
            let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
            request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
            request.sortDescriptors = [
                NSSortDescriptor(keyPath: \CachedRecipe.updatedAt, ascending: false),
                NSSortDescriptor(keyPath: \CachedRecipe.id, ascending: true)
            ]
            var documents: [CachedRecipeSearchDocument] = []
            var droppedRows = false
            for row in try backgroundContext.fetch(request) {
                if let document = searchDocument(from: row) {
                    documents.append(document)
                } else {
                    droppedRows = true
                    backgroundContext.delete(row)
                }
            }
            if droppedRows {
                // A full re-sync rewrites the dropped recipes.
                try backgroundContext.save()
                clearSyncCursor(accountKey: accountKey)
            }
            return documents
        }
    }

    /// Rows written under an older cache schema must never be served: Core
    /// Data's lightweight migration backfills columns the old schema lacked
    /// with defaults (e.g. an empty ingredient match text), so searching them
    /// would silently omit recipes the server would return. The schema bump
    /// already forces a full re-sync; this drops the migrated rows so the
    /// window before that sync completes serves nothing instead of wrong
    /// results. Runs on `backgroundContext`'s queue.
    private func purgeRowsWrittenByOlderSchema(accountKey: String) throws {
        let key = rowsSchemaVersionKey(accountKey: accountKey)
        guard userDefaults.integer(forKey: key) != Self.cacheSchemaVersion else {
            return
        }
        let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
        request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
        let staleRows = try backgroundContext.fetch(request)
        guard !staleRows.isEmpty else {
            return
        }
        for row in staleRows {
            backgroundContext.delete(row)
        }
        try backgroundContext.save()
    }

    func apply(syncResponse: SyncRecipesResponse, accountKey: String) async throws {
        try await backgroundContext.perform { [self] in
            let context = backgroundContext
            // One lookup for the whole page; a fetch per recipe dominated
            // applying a 100-recipe page.
            let pageIds = syncResponse.deleted + syncResponse.recipes.map(\.id)
            let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
            request.predicate = NSPredicate(
                format: "accountKey == %@ AND id IN %@",
                accountKey,
                pageIds as NSArray
            )
            var existingById: [UUID: CachedRecipe] = [:]
            for row in try context.fetch(request) {
                if let id = row.id {
                    existingById[id] = row
                }
            }

            for id in syncResponse.deleted {
                if let cachedRecipe = existingById.removeValue(forKey: id) {
                    context.delete(cachedRecipe)
                }
            }

            for recipe in syncResponse.recipes {
                let cachedRecipe = existingById[recipe.id] ?? CachedRecipe(context: context)
                existingById[recipe.id] = cachedRecipe
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

            if context.hasChanges {
                try context.save()
            }
            userDefaults.set(Self.cacheSchemaVersion, forKey: rowsSchemaVersionKey(accountKey: accountKey))
        }
    }

    /// Nil for a corrupt row, which the caller drops so a re-sync can
    /// replace it; crashing would repeat on every launch.
    private func searchDocument(from cachedRecipe: CachedRecipe) -> CachedRecipeSearchDocument? {
        let rowId = cachedRecipe.id?.uuidString ?? "<no id>"
        guard let createdAt = cachedRecipe.createdAt,
              let id = cachedRecipe.id,
              let ingredientMatchText = cachedRecipe.ingredientMatchText,
              let ingredientsJSON = cachedRecipe.ingredientsJSON,
              let instructions = cachedRecipe.instructions,
              let tagsJSON = cachedRecipe.tagsJSON,
              let title = cachedRecipe.title,
              let updatedAt = cachedRecipe.updatedAt
        else {
            logger.log("Dropping cached recipe \(rowId): missing required fields", source: "RecipeCache")
            return nil
        }
        guard let ingredients = decodeCachedJSON([Ingredient].self, from: ingredientsJSON, field: "ingredients", rowId: rowId),
              let tags = decodeCachedJSON([String].self, from: tagsJSON, field: "tags", rowId: rowId)
        else {
            return nil
        }

        let summary = RecipeSummary(
            createdAt: createdAt,
            description: cachedRecipe.summaryDescription,
            id: id,
            rating: cachedRecipe.rating.flatMap(Int.init),
            tags: tags,
            thumbnailPhotoId: cachedRecipe.thumbnailPhotoId,
            title: title,
            updatedAt: updatedAt
        )
        return CachedRecipeSearchDocument(
            summary: summary,
            ingredients: ingredients,
            ingredientMatchText: ingredientMatchText,
            instructions: instructions,
            notes: cachedRecipe.notes
        )
    }

    private func ingredientsJSON(_ ingredients: [Ingredient]) throws -> String {
        try encodeCachedJSON(ingredients, field: "ingredients")
    }

    private func tagsJSON(_ tags: [String]) throws -> String {
        try encodeCachedJSON(tags, field: "tags")
    }

    private func encodeCachedJSON<T: Encodable>(_ value: T, field: String) throws -> String {
        let data = try JSONEncoder().encode(value)
        guard let json = String(data: data, encoding: .utf8) else {
            throw RecipeCacheError.unencodableField(field)
        }
        return json
    }

    private func decodeCachedJSON<T: Decodable>(_ type: T.Type, from json: String, field: String, rowId: String) -> T? {
        do {
            return try JSONDecoder().decode(type, from: Data(json.utf8))
        } catch {
            logger.log("Dropping cached recipe \(rowId): invalid \(field) JSON: \(error)", source: "RecipeCache")
            return nil
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
