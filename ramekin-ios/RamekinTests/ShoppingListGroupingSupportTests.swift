import XCTest
@testable import Ramekin

final class ShoppingListGroupingSupportTests: XCTestCase {
    func testOrdersPresentCategoriesByServerOrder() {
        let ordered = ShoppingListGroupingSupport.orderedCategories(
            present: ["Other", "Produce", "Cheese"],
            categoryOrder: ["Produce", "Meat & Seafood", "Cheese", "Other"]
        )

        XCTAssertEqual(ordered, ["Produce", "Cheese", "Other"])
    }

    func testSkipsCategoriesWithNoItems() {
        let ordered = ShoppingListGroupingSupport.orderedCategories(
            present: ["Produce"],
            categoryOrder: ["Produce", "Meat & Seafood", "Other"]
        )

        XCTAssertEqual(ordered, ["Produce"])
    }

    func testAppendsUnknownCategoriesAlphabetically() {
        let ordered = ShoppingListGroupingSupport.orderedCategories(
            present: ["Produce", "Zebra Aisle", "Aardvark Aisle"],
            categoryOrder: ["Produce", "Other"]
        )

        XCTAssertEqual(ordered, ["Produce", "Aardvark Aisle", "Zebra Aisle"])
    }

    func testEmptyOrderBeforeFirstSyncShowsAllCategoriesAlphabetically() {
        let ordered = ShoppingListGroupingSupport.orderedCategories(
            present: ["Other", "Produce"],
            categoryOrder: []
        )

        XCTAssertEqual(ordered, ["Other", "Produce"])
    }

    func testEmptyPresentYieldsEmpty() {
        let ordered = ShoppingListGroupingSupport.orderedCategories(
            present: [],
            categoryOrder: ["Produce", "Other"]
        )

        XCTAssertEqual(ordered, [])
    }
}
