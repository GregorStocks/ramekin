import XCTest
@testable import Ramekin

final class TagFilterCacheTests: XCTestCase {
    func testTagsAreIsolatedByUserAndServer() {
        let defaults = UserDefaults(suiteName: defaultsSuiteName)!
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        defaults.removePersistentDomain(forName: defaultsSuiteName)
        let firstTag = makeTag(name: "Dinner")
        let secondUserTag = makeTag(name: "Lunch")
        let secondServerTag = makeTag(name: "Breakfast")

        TagFilterCache.saveSelectedTags([firstTag.name], accountKey: firstAccount, userDefaults: defaults)
        TagFilterCache.saveAvailableTags([firstTag], accountKey: firstAccount, userDefaults: defaults)
        TagFilterCache.saveSelectedTags(
            [secondUserTag.name],
            accountKey: secondUserAccount,
            userDefaults: defaults
        )
        TagFilterCache.saveAvailableTags(
            [secondUserTag],
            accountKey: secondUserAccount,
            userDefaults: defaults
        )
        TagFilterCache.saveSelectedTags(
            [secondServerTag.name],
            accountKey: secondServerAccount,
            userDefaults: defaults
        )
        TagFilterCache.saveAvailableTags(
            [secondServerTag],
            accountKey: secondServerAccount,
            userDefaults: defaults
        )

        XCTAssertEqual(
            TagFilterCache.loadSelectedTags(accountKey: firstAccount, userDefaults: defaults),
            ["Dinner"]
        )
        XCTAssertEqual(
            TagFilterCache.loadAvailableTags(accountKey: firstAccount, userDefaults: defaults).map(\.name),
            ["Dinner"]
        )
        XCTAssertEqual(
            TagFilterCache.loadSelectedTags(accountKey: secondUserAccount, userDefaults: defaults),
            ["Lunch"]
        )
        XCTAssertEqual(
            TagFilterCache.loadAvailableTags(accountKey: secondServerAccount, userDefaults: defaults).map(\.name),
            ["Breakfast"]
        )
    }

    func testLegacyTagsMigrateOnlyToActiveAccount() throws {
        let defaults = UserDefaults(suiteName: defaultsSuiteName)!
        defer { defaults.removePersistentDomain(forName: defaultsSuiteName) }
        defaults.removePersistentDomain(forName: defaultsSuiteName)
        let tag = makeTag(name: "Dinner")
        defaults.set(try JSONEncoder().encode([tag.name]), forKey: "recipeSelectedTags")
        defaults.set(try CodableHelper.jsonEncoder.encode([tag]), forKey: "recipeAvailableTags")

        TagFilterCache.migrateLegacyState(activeAccountKey: firstAccount, userDefaults: defaults)

        XCTAssertEqual(
            TagFilterCache.loadSelectedTags(accountKey: firstAccount, userDefaults: defaults),
            ["Dinner"]
        )
        XCTAssertEqual(
            TagFilterCache.loadAvailableTags(accountKey: firstAccount, userDefaults: defaults).map(\.name),
            ["Dinner"]
        )
        XCTAssertTrue(
            TagFilterCache.loadSelectedTags(accountKey: secondUserAccount, userDefaults: defaults).isEmpty
        )
        XCTAssertNil(defaults.object(forKey: "recipeSelectedTags"))
        XCTAssertNil(defaults.object(forKey: "recipeAvailableTags"))
    }

    private func makeTag(name: String) -> TagItem {
        TagItem(
            createdAt: Date(timeIntervalSince1970: 100),
            id: UUID(),
            name: name,
            namespace: nil,
            recipeCount: 1,
            value: name
        )
    }

    private var defaultsSuiteName: String { "TagFilterCacheTests.\(name)" }
    private var firstAccount: String { AccountScope.key(serverURL: "https://one.test", username: "chef") }
    private var secondUserAccount: String { AccountScope.key(serverURL: "https://one.test", username: "baker") }
    private var secondServerAccount: String { AccountScope.key(serverURL: "https://two.test", username: "chef") }
}
