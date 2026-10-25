import XCTest
@testable import Ramekin

final class SceneRestorationTests: XCTestCase {
    private let first = UUID()
    private let second = UUID()

    func testPathRoundTrips() {
        let path: [NavigationDestination] = [.settings, .recipe(first), .recipe(second)]

        XCTAssertEqual(SceneRestoration.decodePath(SceneRestoration.encodePath(path)), path)
    }

    func testUnreadablePathRestoresAsEmpty() {
        XCTAssertEqual(SceneRestoration.decodePath(Data()), [])
        XCTAssertEqual(SceneRestoration.decodePath(Data("not json".utf8)), [])
    }

    func testPrunedPathStopsAtFirstMissingRecipe() {
        let path: [NavigationDestination] = [.settings, .recipe(first), .recipe(second)]

        XCTAssertEqual(
            SceneRestoration.prunedPath(path) { $0 != self.first },
            [.settings]
        )
        XCTAssertEqual(
            SceneRestoration.prunedPath(path) { $0 != self.second },
            [.settings, .recipe(first)]
        )
        XCTAssertEqual(SceneRestoration.prunedPath(path) { _ in true }, path)
    }

    func testRecipeIdsInPath() {
        let path: [NavigationDestination] = [.recipe(first), .settings, .recipe(second)]

        XCTAssertEqual(SceneRestoration.recipeIds(in: path), [first, second])
    }

    func testScalesRoundTrip() {
        let scales = [
            first: RecipeScaleState(scale: 2, customInput: ""),
            second: RecipeScaleState(scale: 1.5, customInput: "1.5")
        ]

        XCTAssertEqual(SceneRestoration.decodeScales(SceneRestoration.encodeScales(scales)), scales)
        XCTAssertEqual(SceneRestoration.decodeScales(Data("garbage".utf8)), [:])
    }

    func testPrunedScalesKeepsOnlyOpenRecipes() {
        let scales = [
            first: RecipeScaleState(scale: 2, customInput: ""),
            second: RecipeScaleState(scale: 3, customInput: "")
        ]

        XCTAssertEqual(
            SceneRestoration.prunedScales(scales, keeping: [second]),
            [second: RecipeScaleState(scale: 3, customInput: "")]
        )
    }
}
