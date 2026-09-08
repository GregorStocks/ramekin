import XCTest
@testable import Ramekin

final class CalorieEstimateViewModelTests: XCTestCase {
    @MainActor
    func testPassesOriginalIngredientsServingsAndScaleAndDisplaysServerResult() async {
        let ingredient = Ingredient(item: "yogurt", measurements: [Measurement(amount: "100", unit: "g")])
        let request = EstimateCaloriesRequest(ingredients: [ingredient], scale: 2, servings: "4")
        let result = CalorieEstimateResponse(
            databaseVersion: "test",
            summary: "Whole-recipe calories unknown: yogurt.",
            unknownIngredients: [UnknownCalorieIngredient(index: 0, item: "yogurt", reason: "Ambiguous ingredient")]
        )
        let model = CalorieEstimateViewModel { actual in
            XCTAssertEqual(actual, request)
            return result
        }
        await model.load(request)
        XCTAssertEqual(model.response, result)
        XCTAssertEqual(model.request, request)
        XCTAssertNil(model.error)
        XCTAssertFalse(model.isLoading)
    }

    @MainActor
    func testFailureClearsPreviousEstimateAndRetryRecovers() async {
        let request = EstimateCaloriesRequest(ingredients: [], scale: 1)
        let result = CalorieEstimateResponse(databaseVersion: "test", summary: "No ingredients to estimate.", unknownIngredients: [])
        var shouldFail = false
        let model = CalorieEstimateViewModel { _ in
            if shouldFail { throw URLError(.notConnectedToInternet) }
            return result
        }
        await model.load(request)
        XCTAssertEqual(model.response, result)
        shouldFail = true
        await model.load(EstimateCaloriesRequest(ingredients: [], scale: 2))
        XCTAssertNil(model.response)
        XCTAssertNotNil(model.error)
        XCTAssertFalse(model.isLoading)
        shouldFail = false
        await model.load(request)
        XCTAssertEqual(model.response, result)
        XCTAssertNil(model.error)
    }

    @MainActor
    func testLateResponseCannotReplaceNewScaleOrRecipe() async {
        var continuation: CheckedContinuation<CalorieEstimateResponse, Never>?
        let newer = CalorieEstimateResponse(databaseVersion: "test", summary: "New recipe", unknownIngredients: [])
        let model = CalorieEstimateViewModel { request in
            if request.scale == 1 {
                return await withCheckedContinuation { continuation = $0 }
            }
            return newer
        }
        let first = Task { await model.load(EstimateCaloriesRequest(ingredients: [], scale: 1)) }
        while continuation == nil { await Task.yield() }
        XCTAssertTrue(model.isLoading)
        XCTAssertNil(model.response)
        await model.load(EstimateCaloriesRequest(ingredients: [], scale: 2))
        XCTAssertEqual(model.response, newer)
        continuation?.resume(returning: CalorieEstimateResponse(databaseVersion: "test", summary: "Old recipe", unknownIngredients: []))
        await first.value
        XCTAssertEqual(model.response, newer)
        XCTAssertEqual(model.request?.scale, 2)
    }
}
