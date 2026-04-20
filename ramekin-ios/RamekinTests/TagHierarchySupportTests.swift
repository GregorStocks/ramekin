import XCTest
@testable import Ramekin

final class TagHierarchySupportTests: XCTestCase {
    func testParseSplitsHierarchicalTags() {
        let parsed = TagHierarchySupport.parse(name: " ingredient:chicken ")

        XCTAssertEqual(
            parsed,
            TagHierarchySupport.ParsedTag(
                name: "ingredient:chicken",
                namespace: "ingredient",
                value: "chicken"
            )
        )
    }

    func testNormalizedNamespaceLowercasesAndRejectsInvalidValues() {
        XCTAssertEqual(
            TagHierarchySupport.normalizedNamespace(from: " Course "),
            "course"
        )
        XCTAssertNil(TagHierarchySupport.normalizedNamespace(from: "bad namespace"))
        XCTAssertNil(TagHierarchySupport.normalizedNamespace(from: "1course"))
    }

    func testFormattedNameBuildsHierarchicalAndFlatTags() {
        XCTAssertEqual(
            TagHierarchySupport.formattedName(namespace: "ingredient", value: "Chicken"),
            "ingredient:Chicken"
        )
        XCTAssertEqual(
            TagHierarchySupport.formattedName(namespace: nil, value: "Dinner"),
            "Dinner"
        )
        XCTAssertNil(TagHierarchySupport.formattedName(namespace: "bad namespace", value: "Chicken"))
    }

    func testGroupsOrderSeededExtrasThenUncategorized() {
        let tags = [
            TagItem(createdAt: Date(), id: UUID(), name: "season:winter", namespace: "season", recipeCount: 1, value: "winter"),
            TagItem(createdAt: Date(), id: UUID(), name: "occasion:holiday", namespace: "occasion", recipeCount: 1, value: "holiday"),
            TagItem(createdAt: Date(), id: UUID(), name: "dinner", namespace: nil, recipeCount: 1, value: "dinner"),
            TagItem(createdAt: Date(), id: UUID(), name: "ingredient:chicken", namespace: "ingredient", recipeCount: 1, value: "chicken")
        ]

        let groups = TagHierarchySupport.groups(for: tags)

        XCTAssertEqual(groups.map(\.title), ["ingredient", "season", "occasion", "Uncategorized"])
        XCTAssertEqual(groups[0].items.map(\.name), ["ingredient:chicken"])
        XCTAssertEqual(groups[3].items.map(\.name), ["dinner"])
    }
}
