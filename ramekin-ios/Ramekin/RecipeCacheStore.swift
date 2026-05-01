import CoreData
import Foundation

@MainActor
final class RecipeCacheStore {
    static let shared = RecipeCacheStore()

    private let coreDataStack: CoreDataStack
    private let userDefaults: UserDefaults

    init(coreDataStack: CoreDataStack = .shared, userDefaults: UserDefaults = .standard) {
        self.coreDataStack = coreDataStack
        self.userDefaults = userDefaults
    }

    func currentAccountKey() -> String? {
        guard let serverURL = KeychainHelper.shared.getServerURL(),
              let username = KeychainHelper.shared.getUsername()
        else {
            return nil
        }
        return "\(serverURL)|\(username)"
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
        let request = NSFetchRequest<CachedRecipe>(entityName: "CachedRecipe")
        request.predicate = NSPredicate(format: "accountKey == %@", accountKey)
        request.sortDescriptors = [
            NSSortDescriptor(keyPath: \CachedRecipe.updatedAt, ascending: false),
            NSSortDescriptor(keyPath: \CachedRecipe.id, ascending: true)
        ]
        return try coreDataStack.viewContext.fetch(request).map(recipeSummary)
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

    private func recipeSummary(from cachedRecipe: CachedRecipe) -> RecipeSummary {
        guard let createdAt = cachedRecipe.createdAt,
              let id = cachedRecipe.id,
              let tagsJSON = cachedRecipe.tagsJSON,
              let title = cachedRecipe.title,
              let updatedAt = cachedRecipe.updatedAt
        else {
            fatalError("CachedRecipe is missing required fields")
        }

        return RecipeSummary(
            createdAt: createdAt,
            description: cachedRecipe.summaryDescription,
            id: id,
            rating: cachedRecipe.rating.flatMap(Int.init),
            tags: tags(from: tagsJSON),
            thumbnailPhotoId: cachedRecipe.thumbnailPhotoId,
            title: title,
            updatedAt: updatedAt
        )
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
        let encodedAccountKey = Data(accountKey.utf8).base64EncodedString()
        return "recipe_cache_last_sync_at_\(encodedAccountKey)"
    }
}
