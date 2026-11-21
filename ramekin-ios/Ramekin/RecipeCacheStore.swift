import CoreData
import Foundation

struct CachedRecipeSearchDocument {
    let summary: RecipeSummary
    let ingredients: [Ingredient]
    /// The server's stored JSON text of the ingredients — the
    /// exact haystack server search matches bare text against. Distinct from
    /// `ingredients`, which the relevance scorer flattens itself.
    let ingredientMatchText: String
    let instructions: String
    let notes: String?
    /// Facts only the server knows, synced so the filters over them run
    /// locally: what `source:`, `calories:` (and the calorie sort), and
    /// `photo_size:`/`photo_dim:` compare.
    var sourceName: String?
    var kcalPerServing: Double?
    var photos: [SyncPhoto] = []
}

/// Where the recipe cache stands in the server's change stream: what the
/// last applied sync page said to ask for next.
struct RecipeSyncPosition: Codable, Equatable {
    let cursor: Int64
    /// Set while the sync that produced it still had pages to fetch.
    var afterId: UUID?
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
    // v5 added the source name, calorie estimate and photo facts; bumping
    // forces a full re-sync so every cached recipe carries them.
    private static let cacheSchemaVersion = 5

    /// Only touched inside `perform`.
    private let backgroundContext: NSManagedObjectContext
    /// The corpus `loadSearchCorpus` last built, until `apply` changes the
    /// cache. Only touched inside `perform`.
    private var preparedCorpus: (accountKey: String, corpus: RecipeSearchCorpus)?
    private let userDefaults: UserDefaults
    private let logger = DebugLogger.shared

    init(coreDataStack: CoreDataStack = .shared, userDefaults: UserDefaults = .standard) {
        backgroundContext = coreDataStack.newBackgroundContext()
        self.userDefaults = coreDataStack.syncStateDefaults(userDefaults)
    }

    func currentAccountKey() -> String? {
        AccountScope.currentAccountKey()
    }

    func syncPosition(accountKey: String) -> RecipeSyncPosition? {
        guard let data = userDefaults.data(forKey: syncPositionKey(accountKey: accountKey)) else {
            return nil
        }
        do {
            return try JSONDecoder().decode(RecipeSyncPosition.self, from: data)
        } catch {
            // Dropping it only costs a full sync.
            logger.log("Discarding invalid sync position: \(error)", source: "RecipeCache")
            clearSyncPosition(accountKey: accountKey)
            return nil
        }
    }

    func clearSyncPosition(accountKey: String) {
        userDefaults.removeObject(forKey: syncPositionKey(accountKey: accountKey))
    }

    func loadSearchDocuments(accountKey: String) async throws -> [CachedRecipeSearchDocument] {
        try await backgroundContext.perform { [self] in
            try fetchSearchDocuments(accountKey: accountKey)
        }
    }

    /// The account's cached recipes prepared for searching. Reading and
    /// normalizing the whole cookbook is the expensive part of a search, so
    /// the corpus is kept until `apply` changes the cache: typing a query
    /// then costs a scan, not a reload.
    func loadSearchCorpus(accountKey: String) async throws -> RecipeSearchCorpus {
        try await backgroundContext.perform { [self] in
            let purged = try purgeRowsWrittenByOlderSchema(accountKey: accountKey)
            if !purged, let preparedCorpus, preparedCorpus.accountKey == accountKey {
                return preparedCorpus.corpus
            }
            preparedCorpus = nil
            let start = CFAbsoluteTimeGetCurrent()
            let corpus = RecipeSearchCorpus(documents: try fetchSearchDocuments(accountKey: accountKey))
            let elapsed = String(format: "%.2f", CFAbsoluteTimeGetCurrent() - start)
            logger.log("Built search corpus of \(corpus.entries.count) recipes (\(elapsed)s)", source: "RecipeCache")
            preparedCorpus = (accountKey, corpus)
            // The list can show as soon as this returns; get the text ready
            // for the first keystroke meanwhile.
            Task.detached(priority: .utility) { corpus.prepareSearchTexts() }
            return corpus
        }
    }

    /// Runs on `backgroundContext`'s queue.
    private func fetchSearchDocuments(accountKey: String) throws -> [CachedRecipeSearchDocument] {
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
            clearSyncPosition(accountKey: accountKey)
        }
        return documents
    }

    /// Which of `ids` the cache holds for the account, or nil while a sync
    /// has yet to reach the end of the stream (first launch, a schema bump,
    /// an interrupted sync), when a missing recipe may just not be synced
    /// yet.
    func existingRecipeIds(_ ids: [UUID], accountKey: String) async throws -> Set<UUID>? {
        guard let position = syncPosition(accountKey: accountKey), position.afterId == nil else {
            return nil
        }
        return try await backgroundContext.perform { [self] in
            let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
            request.predicate = NSPredicate(
                format: "accountKey == %@ AND id IN %@",
                accountKey,
                ids as [NSUUID]
            )
            return Set(try backgroundContext.fetch(request).compactMap(\.id))
        }
    }

    /// Rows written under an older cache schema must never be served: Core
    /// Data's lightweight migration backfills columns the old schema lacked
    /// with defaults (e.g. an empty ingredient match text), so searching them
    /// would silently omit recipes the server would return. The schema bump
    /// already forces a full re-sync; this drops the migrated rows so the
    /// window before that sync completes serves nothing instead of wrong
    /// results. Returns whether it dropped any. Runs on `backgroundContext`'s
    /// queue.
    @discardableResult
    private func purgeRowsWrittenByOlderSchema(accountKey: String) throws -> Bool {
        let key = rowsSchemaVersionKey(accountKey: accountKey)
        guard userDefaults.integer(forKey: key) != Self.cacheSchemaVersion else {
            return false
        }
        let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
        request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
        let staleRows = try backgroundContext.fetch(request)
        guard !staleRows.isEmpty else {
            return false
        }
        for row in staleRows {
            backgroundContext.delete(row)
        }
        try backgroundContext.save()
        return true
    }

    /// Applies one sync page and records the position it leaves the cache
    /// at. The position is written after the rows, so an interruption in
    /// between only has the page delivered again.
    func apply(syncResponse: SyncRecipesResponse, accountKey: String) async throws {
        try await backgroundContext.perform { [self] in
            let context = backgroundContext
            let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
            if syncResponse.fullResync {
                // The server is starting the stream over and sends no
                // tombstones for what it no longer has, so nothing cached
                // can be trusted to still exist.
                request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
            } else {
                // One lookup for the whole page; a fetch per recipe dominated
                // applying a 100-recipe page.
                let pageIds = syncResponse.deleted + syncResponse.changes.map(\.id)
                request.predicate = NSPredicate(
                    format: "accountKey == %@ AND id IN %@",
                    accountKey,
                    pageIds as NSArray
                )
            }
            var existingById: [UUID: CachedRecipe] = [:]
            for row in try context.fetch(request) {
                if let id = row.id {
                    existingById[id] = row
                }
            }

            let live = Set(syncResponse.changes.map(\.id))
            let gone = syncResponse.fullResync
                ? existingById.keys.filter { !live.contains($0) }
                : syncResponse.deleted
            for id in gone {
                if let cachedRecipe = existingById.removeValue(forKey: id) {
                    context.delete(cachedRecipe)
                }
            }

            for recipe in syncResponse.changes {
                let cachedRecipe = existingById[recipe.id] ?? CachedRecipe(context: context)
                existingById[recipe.id] = cachedRecipe
                try write(recipe, accountKey: accountKey, to: cachedRecipe)
            }

            if context.hasChanges {
                preparedCorpus = nil
                try context.save()
            }
            userDefaults.set(Self.cacheSchemaVersion, forKey: rowsSchemaVersionKey(accountKey: accountKey))
            let position = RecipeSyncPosition(cursor: syncResponse.cursor, afterId: syncResponse.afterId)
            userDefaults.set(try JSONEncoder().encode(position), forKey: syncPositionKey(accountKey: accountKey))
        }
    }

    private func write(_ recipe: SyncRecipe, accountKey: String, to cachedRecipe: CachedRecipe) throws {
        cachedRecipe.accountKey = accountKey
        cachedRecipe.id = recipe.id
        cachedRecipe.ingredientMatchText = recipe.ingredientMatchText
        cachedRecipe.ingredientsJSON = try ingredientsJSON(recipe.ingredients)
        cachedRecipe.instructions = recipe.instructions
        cachedRecipe.notes = recipe.notes
        cachedRecipe.sourceName = recipe.sourceName
        cachedRecipe.kcalPerServing = recipe.kcalPerServing.map(NSNumber.init(value:))
        cachedRecipe.caloriesLabel = recipe.caloriesPerServing
        cachedRecipe.photosJSON = try encodeCachedJSON(recipe.photos, field: "photos")
        cachedRecipe.title = recipe.title
        cachedRecipe.summaryDescription = recipe.description
        cachedRecipe.tagsJSON = try tagsJSON(recipe.tags)
        cachedRecipe.thumbnailPhotoId = recipe.thumbnailPhotoId
        cachedRecipe.rating = recipe.rating.map(String.init)
        cachedRecipe.createdAt = recipe.createdAt
        cachedRecipe.updatedAt = recipe.updatedAt
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
              let photosJSON = cachedRecipe.photosJSON,
              let tagsJSON = cachedRecipe.tagsJSON,
              let title = cachedRecipe.title,
              let updatedAt = cachedRecipe.updatedAt
        else {
            logger.log("Dropping cached recipe \(rowId): missing required fields", source: "RecipeCache")
            return nil
        }
        guard let ingredients = decodeCachedJSON([Ingredient].self, from: ingredientsJSON, field: "ingredients", rowId: rowId),
              let tags = decodeCachedJSON([String].self, from: tagsJSON, field: "tags", rowId: rowId),
              let photos = decodeCachedJSON([SyncPhoto].self, from: photosJSON, field: "photos", rowId: rowId)
        else {
            return nil
        }

        let summary = RecipeSummary(
            caloriesPerServing: cachedRecipe.caloriesLabel,
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
            notes: cachedRecipe.notes,
            sourceName: cachedRecipe.sourceName,
            kcalPerServing: cachedRecipe.kcalPerServing?.doubleValue,
            photos: photos
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

    private func syncPositionKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(
            prefix: "recipe_cache_v\(Self.cacheSchemaVersion)_sync_position",
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
