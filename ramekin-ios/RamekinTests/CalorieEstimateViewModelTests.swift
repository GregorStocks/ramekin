import Combine
import XCTest
@testable import Ramekin

final class CalorieEstimateViewModelTests: XCTestCase {
    private func estimate(
        _ headline: String,
        status: CalorieStatus = .complete,
        resolving: Bool = false
    ) -> CalorieEstimateResponse {
        CalorieEstimateResponse(
            databaseVersion: "test",
            headline: headline,
            lines: [],
            notCounted: [],
            resolving: resolving,
            status: status
        )
    }

    @MainActor
    func testPassesOriginalIngredientsServingsAndScaleAndDisplaysServerResult() async {
        let ingredient = Ingredient(item: "yogurt", measurements: [Measurement(amount: "100", unit: "g")])
        let request = EstimateCaloriesRequest(ingredients: [ingredient], scale: 2, servings: "4")
        let result = CalorieEstimateResponse(
            databaseVersion: "test",
            headline: "Not enough ingredient data to estimate calories",
            lines: [CalorieLine(index: 0, item: "yogurt", text: "Could be several foods")],
            notCounted: [],
            resolving: false,
            secondary: "1 ingredient couldn't be counted.",
            status: .insufficient
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
        let result = estimate("No ingredients to estimate", status: .empty)
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
        let newer = estimate("New recipe")
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
        continuation?.resume(returning: estimate("Old recipe"))
        await first.value
        XCTAssertEqual(model.response, newer)
        XCTAssertEqual(model.request?.scale, 2)
    }

    @MainActor
    func testPollsQuietlyWhileNamesAreResolving() async {
        let request = EstimateCaloriesRequest(ingredients: [], scale: 1)
        var calls = 0
        var loadingSeenDuringPoll = false
        var model: CalorieEstimateViewModel!
        model = CalorieEstimateViewModel(pollInterval: .zero) { _ in
            calls += 1
            if calls > 1, model.isLoading { loadingSeenDuringPoll = true }
            return calls < 3
                ? self.estimate("Not yet", resolving: true)
                : self.estimate("Counted")
        }
        await model.load(request)
        XCTAssertEqual(calls, 3)
        XCTAssertEqual(model.response?.headline, "Counted")
        XCTAssertFalse(loadingSeenDuringPoll, "polls keep the current estimate on screen")
        XCTAssertFalse(model.isLoading)
    }

    @MainActor
    func testPollReturningTheSameEstimateDoesNotRepublishIt() async {
        let request = EstimateCaloriesRequest(ingredients: [], scale: 1)
        var calls = 0
        let model = CalorieEstimateViewModel(pollInterval: .zero) { _ in
            calls += 1
            return calls < 4
                ? self.estimate("Not yet", resolving: true)
                : self.estimate("Counted")
        }
        var published: [String?] = []
        let subscription = model.$response.dropFirst().sink { published.append($0?.headline) }
        await model.load(request)
        subscription.cancel()
        XCTAssertEqual(calls, 4)
        // Cleared at the start of the load, the first estimate, then only the change.
        XCTAssertEqual(published, [nil, "Not yet", "Counted"])
    }

    @MainActor
    func testPollingStopsWhenANewerRequestReplacesIt() async {
        var calls = 0
        var model: CalorieEstimateViewModel!
        model = CalorieEstimateViewModel(pollInterval: .zero) { request in
            calls += 1
            if request.scale == 1 {
                if calls > 1 {
                    // A newer request arrives while the first is polling.
                    await model.load(EstimateCaloriesRequest(ingredients: [], scale: 2))
                }
                return self.estimate("Old", resolving: true)
            }
            return self.estimate("New")
        }
        await model.load(EstimateCaloriesRequest(ingredients: [], scale: 1))
        XCTAssertEqual(model.response?.headline, "New")
        XCTAssertEqual(model.request?.scale, 2)
    }
}
