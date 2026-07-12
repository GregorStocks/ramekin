import CoreData
import Foundation

struct CachedRecipeSearchDocument {
    let summary: RecipeSummary
    let ingredients: [Ingredient]
    let instructions: String
    let notes: String?
}

@MainActor
final class RecipeCacheStore {
    static let shared = RecipeCacheStore()
    private static let cacheSchemaVersion = 2

    private let coreDataStack: CoreDataStack
    private let userDefaults: UserDefaults

    init(coreDataStack: CoreDataStack = .shared, userDefaults: UserDefaults = .standard) {
        self.coreDataStack = coreDataStack
        self.userDefaults = userDefaults
    }

    func currentAccountKey() -> String? {
        AccountScope.currentAccountKey()
    }

    func lastSyncAt(accountKey: String) -> Date? {
        userDefaults.object(forKey: lastSyncKey(accountKey: accountKey)) as? Date
    }

    func setLastSyncAt(_ date: Date, accountKey: String) {
        userDefaults.set(date, forKey: lastSyncKey(accountKey: accountKey))
    }

    func clearLastSyncAt(accountKey: String) {
        userDefaults.removeObject(forKey: lastSyncKey(accountKey: accountKey))
    }

    func loadRecipes(accountKey: String) throws -> [RecipeSummary] {
        try loadSearchDocuments(accountKey: accountKey).map(\.summary)
    }

    func loadSearchDocuments(accountKey: String) throws -> [CachedRecipeSearchDocument] {
        let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
        request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
        request.sortDescriptors = [
            NSSortDescriptor(keyPath: \CachedRecipe.updatedAt, ascending: false),
            NSSortDescriptor(keyPath: \CachedRecipe.id, ascending: true)
        ]
        return try coreDataStack.viewContext.fetch(request).map(searchDocument)
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
        setLastSyncAt(syncResponse.syncTimestamp, accountKey: accountKey)
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

    private func lastSyncKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(
            prefix: "recipe_cache_v\(Self.cacheSchemaVersion)_last_sync_at",
            accountKey: accountKey
        )
    }
}
