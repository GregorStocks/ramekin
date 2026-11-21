import CoreData
import XCTest
@testable import Ramekin

/// Synthetic cookbook shaped like a real one: ~1.5 KB of instructions, a
/// dozen structured ingredients, and a JSON ingredient match text per recipe.
enum SearchBenchmarkCorpus {
    private static let words = [
        "chicken", "garlic", "onion", "butter", "simmer", "whisk", "roast", "crème", "jalapeño",
        "tomato", "basil", "oven", "minutes", "until", "golden", "stir", "pepper", "salt", "olive",
        "flour", "sugar", "bake", "chop", "fold", "sauté", "broth", "lemon", "thyme", "paprika",
        "noodles", "rice", "beans", "cheese", "cream", "skillet", "medium", "heat", "combine"
    ]

    private static func text(_ seed: Int, wordCount: Int) -> String {
        var state = UInt64(truncatingIfNeeded: seed &* 2_654_435_761 &+ 12_345)
        var parts: [String] = []
        parts.reserveCapacity(wordCount)
        for _ in 0..<wordCount {
            state = state &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
            parts.append(words[Int((state >> 33) % UInt64(words.count))])
        }
        return parts.joined(separator: " ")
    }

    static func recipes(count: Int) -> [SyncRecipe] {
        (0..<count).map { index in
            let ingredients = (0..<12).map { slot in
                Ingredient(
                    item: text(index * 31 + slot, wordCount: 2),
                    measurements: [Measurement(amount: "\(slot + 1)", unit: "cups")],
                    note: slot % 3 == 0 ? "finely chopped" : nil,
                    section: slot < 6 ? "Main" : "Sauce"
                )
            }
            let matchText = "[" + ingredients.map {
                "{\"item\": \"\($0.item)\", \"note\": \"\($0.note ?? "")\", \"section\": \"\($0.section ?? "")\","
                    + " \"measurements\": [{\"unit\": \"cups\", \"amount\": \"1\"}], \"raw\": \"1 cups \($0.item)\"}"
            }.joined(separator: ", ") + "]"
            return SyncRecipe(
                createdAt: Date(timeIntervalSince1970: TimeInterval(1_000_000 + index * 3_600)),
                description: text(index + 7, wordCount: 25),
                id: UUID(),
                ingredientMatchText: matchText,
                ingredients: ingredients,
                instructions: text(index + 99, wordCount: 230),
                notes: index % 2 == 0 ? text(index + 5, wordCount: 30) : nil,
                photos: [],
                rating: index % 6 == 0 ? nil : index % 5 + 1,
                tags: ["Dinner", "cuisine:\(words[index % words.count])"],
                thumbnailPhotoId: index % 4 == 0 ? nil : UUID(),
                title: text(index + 3, wordCount: 4),
                updatedAt: Date(timeIntervalSince1970: TimeInterval(2_000_000 + index * 60))
            )
        }
    }
}

/// Prints where a search over a 1,000-recipe cookbook spends its time. Not
/// an assertion on wall-clock numbers (CI machines vary); run it by hand
/// when touching the search path:
/// `xcodebuild test ... -only-testing:RamekinTests/SearchPerformanceBenchmarkTests`
final class SearchPerformanceBenchmarkTests: XCTestCase {
    private let accountKey = "https://example.test|bench"
    private var report: [String] = []

    func testBenchmarkSearchOver1000Recipes() async throws {
        let suiteName = "SearchPerformanceBenchmarkTests"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("search-bench-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = try makeOnDiskStore(in: directory, defaults: defaults)

        try await store.apply(
            syncResponse: SyncRecipesResponse(
                changes: SearchBenchmarkCorpus.recipes(count: 1_000),
                cursor: 1,
                deleted: [],
                fullResync: false,
                hasMore: false,
                normalizationContractVersion: SearchNormalizationSupport.contractVersion
            ),
            afterId: nil,
            accountKey: accountKey
        )

        var start = Date()
        var corpus = try await store.loadSearchCorpus(accountKey: accountKey)
        record("load corpus", start)
        start = Date()
        corpus.prepareSearchTexts()
        record("normalize corpus text", start)
        start = Date()
        corpus = try await store.loadSearchCorpus(accountKey: accountKey)
        record("reuse corpus", start)
        XCTAssertEqual(corpus.entries.count, 1_000)

        for query in ["c", "chicken", "chicken garlic", "creme jalapeno simmer", "zzzz", ""] {
            time("rank \"\(query)\"", corpus: corpus, filterState: RecipeListFilterState(searchText: query), sortOrder: .newest)
        }
        for sortOrder in [RecipeSortOrder.title, .rating] {
            time("browse by \(sortOrder.rawValue)", corpus: corpus, filterState: RecipeListFilterState(), sortOrder: sortOrder)
        }

        print("SEARCH-BENCH\n" + report.map { "SEARCH-BENCH " + $0 }.joined(separator: "\n"))
    }

    private func makeOnDiskStore(in directory: URL, defaults: UserDefaults) throws -> RecipeCacheStore {
        let container = CoreDataStack.makeContainer()
        let description = NSPersistentStoreDescription(url: directory.appendingPathComponent("bench.sqlite"))
        description.shouldAddStoreAsynchronously = false
        container.persistentStoreDescriptions = [description]
        var loadError: Error?
        container.loadPersistentStores { loadError = $1 }
        if let loadError {
            throw loadError
        }
        return RecipeCacheStore(coreDataStack: CoreDataStack(container: container), userDefaults: defaults)
    }

    private func time(
        _ label: String,
        corpus: RecipeSearchCorpus,
        filterState: RecipeListFilterState,
        sortOrder: RecipeSortOrder
    ) {
        let start = Date()
        let visible = RecipeSummaryCacheSupport.visibleRecipes(corpus: corpus, filterState: filterState, sortOrder: sortOrder)
        record("\(label) (\(visible.count) hits)", start)
    }

    private func record(_ label: String, _ start: Date) {
        report.append(String(format: "%@: %.1f ms", label, Date().timeIntervalSince(start) * 1_000))
    }
}
