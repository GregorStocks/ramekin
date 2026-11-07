import XCTest
@testable import Ramekin

final class ShareExtensionTests: XCTestCase {
    // MARK: - URL Validation Tests

    func testValidRecipeURLs() {
        let validURLs = [
            "https://www.allrecipes.com/recipe/12345/chocolate-cake",
            "https://cooking.nytimes.com/recipes/1234-pasta",
            "https://www.seriouseats.com/best-chocolate-chip-cookies",
            "https://www.bonappetit.com/recipe/chocolate-cake",
            "https://example.com/my-recipe"
        ]

        for urlString in validURLs {
            let url = URL(string: urlString)
            XCTAssertNotNil(url, "Should parse valid URL: \(urlString)")
            XCTAssertTrue(url!.scheme == "https" || url!.scheme == "http",
                         "URL should have http(s) scheme: \(urlString)")
        }
    }

    func testURLWithQueryParameters() {
        let urlString = "https://example.com/recipe?id=123&source=share"
        let url = URL(string: urlString)

        XCTAssertNotNil(url)
        XCTAssertEqual(url?.host, "example.com")
        XCTAssertEqual(url?.path, "/recipe")
        XCTAssertNotNil(url?.query)
    }

    func testURLWithFragment() {
        let urlString = "https://example.com/recipe#ingredients"
        let url = URL(string: urlString)

        XCTAssertNotNil(url)
        XCTAssertEqual(url?.fragment, "ingredients")
    }

    func testInternationalURLs() {
        let urlString = "https://example.com/recette/gâteau"
        let url = URL(string: urlString.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? urlString)

        XCTAssertNotNil(url)
    }

    // MARK: - Share Capture State Machine

    @MainActor
    func testRepeatedStartSendsTheCaptureOnce() async {
        let harness = ShareHarness()
        harness.model.start()
        harness.model.start()

        await waitUntil { harness.gate.calls == 1 }
        await settle()
        XCTAssertEqual(harness.gate.calls, 1)
        XCTAssertEqual(harness.model.status, .sending(slow: false))
    }

    @MainActor
    func testSlowAffordanceAppearsWhileTheAttemptIsStillSending() async {
        let harness = ShareHarness()
        harness.model.start()
        await waitUntil { harness.clock.pending(ShareCaptureModel.slowAffordanceDelay) == 1 }

        harness.clock.fireFirst(ShareCaptureModel.slowAffordanceDelay)

        await waitUntil { harness.model.status == .sending(slow: true) }
    }

    @MainActor
    func testSlowTimerFromAFailedAttemptDoesNotFireDuringTheRetry() async {
        let harness = ShareHarness()
        let slow = ShareCaptureModel.slowAffordanceDelay
        harness.model.start()
        await waitUntil { harness.gate.calls == 1 && harness.clock.pending(slow) == 1 }
        harness.gate.fail(URLError(.notConnectedToInternet))
        await waitUntil { harness.model.status.isFailed }

        harness.model.retry()
        await waitUntil { harness.gate.calls == 2 && harness.clock.pending(slow) == 2 }

        // The first attempt's timer runs out while the retry is in flight.
        harness.clock.fireFirst(slow)
        await settle()
        XCTAssertEqual(harness.model.status, .sending(slow: false))

        // The retry's own timer still shows the affordance.
        harness.clock.fireFirst(slow)
        await waitUntil { harness.model.status == .sending(slow: true) }
    }

    @MainActor
    func testRetryIsIgnoredUnlessTheLastAttemptFailed() async {
        let harness = ShareHarness()
        harness.model.start()
        await waitUntil { harness.gate.calls == 1 }

        harness.model.retry()
        await settle()

        XCTAssertEqual(harness.gate.calls, 1)
    }

    @MainActor
    func testDoneAfterSuccessCompletesOnlyOnce() async {
        let harness = ShareHarness()
        harness.model.start()
        await waitUntil { harness.gate.calls == 1 }
        harness.gate.succeed()
        await waitUntil { harness.model.status == .success }
        await waitUntil { harness.clock.pending(ShareCaptureModel.successDismissDelay) == 1 }

        harness.model.complete()
        harness.clock.fireFirst(ShareCaptureModel.successDismissDelay)
        await settle()
        harness.model.cancel()

        XCTAssertEqual(harness.completions, 1)
        XCTAssertEqual(harness.cancellations, 0)
    }

    @MainActor
    func testSuccessClosesTheSheetAfterTheDelay() async {
        let harness = ShareHarness()
        harness.model.start()
        await waitUntil { harness.gate.calls == 1 }
        harness.gate.succeed()
        await waitUntil { harness.clock.pending(ShareCaptureModel.successDismissDelay) == 1 }

        harness.clock.fireFirst(ShareCaptureModel.successDismissDelay)

        await waitUntil { harness.completions == 1 }
    }

    @MainActor
    func testCancelStopsTheSendAndIgnoresItsResult() async {
        let harness = ShareHarness()
        harness.model.start()
        await waitUntil { harness.gate.calls == 1 }

        harness.model.cancel()
        XCTAssertEqual(harness.model.sendTask?.isCancelled, true)
        harness.gate.succeed()
        await settle()

        XCTAssertEqual(harness.cancellations, 1)
        XCTAssertEqual(harness.completions, 0)
        XCTAssertEqual(harness.model.status, .sending(slow: false))
    }

    @MainActor
    func testSignedOutShowsNotLoggedInWithoutSending() async {
        let harness = ShareHarness(isLoggedIn: false)
        harness.model.start()
        await settle()

        XCTAssertEqual(harness.model.status, .notLoggedIn)
        XCTAssertEqual(harness.gate.calls, 0)
    }

    @MainActor
    func testMissingPayloadFails() {
        let harness = ShareHarness(payload: nil)
        harness.model.start()

        XCTAssertEqual(harness.model.status, .failed(ShareCaptureModel.missingPayloadMessage))
        XCTAssertEqual(harness.gate.calls, 0)
    }

    @MainActor
    func testOnly401MeansSignedOut() {
        XCTAssertEqual(
            ShareCaptureModel.outcome(for: RamekinAPI.APIError.httpError(401, .unauthorized, "nope")),
            .notLoggedIn
        )
        XCTAssertEqual(ShareCaptureModel.outcome(for: RamekinAPI.APIError.noAuthToken), .notLoggedIn)
        XCTAssertEqual(
            ShareCaptureModel.outcome(for: RamekinAPI.APIError.httpError(403, nil, "<html>")),
            .failed(ShareCaptureModel.forbiddenMessage)
        )
        XCTAssertEqual(
            ShareCaptureModel.outcome(for: RamekinAPI.APIError.httpError(500, nil, "boom")),
            .failed("boom")
        )
    }
}

private extension ShareCaptureModel.Status {
    var isFailed: Bool {
        if case .failed = self { return true }
        return false
    }
}

/// A fake clock whose sleeps end only when the test fires them.
@MainActor
private final class ManualClock {
    private var waiters: [(delay: TimeInterval, continuation: CheckedContinuation<Void, Never>)] = []

    func sleep(_ delay: TimeInterval) async {
        await withCheckedContinuation { waiters.append((delay, $0)) }
    }

    func pending(_ delay: TimeInterval) -> Int {
        waiters.filter { $0.delay == delay }.count
    }

    func fireFirst(_ delay: TimeInterval) {
        guard let index = waiters.firstIndex(where: { $0.delay == delay }) else {
            XCTFail("No pending sleep of \(delay)s")
            return
        }
        waiters.remove(at: index).continuation.resume()
    }
}

/// A capture whose attempts end only when the test says how.
@MainActor
private final class CaptureGate {
    private(set) var calls = 0
    private var pending: [CheckedContinuation<Void, Error>] = []

    func capture() async throws {
        calls += 1
        try await withCheckedThrowingContinuation { pending.append($0) }
    }

    func succeed() {
        pending.removeFirst().resume()
    }

    func fail(_ error: Error) {
        pending.removeFirst().resume(throwing: error)
    }
}

@MainActor
private final class ShareHarness {
    let clock = ManualClock()
    let gate = CaptureGate()
    private(set) var completions = 0
    private(set) var cancellations = 0
    private(set) var model: ShareCaptureModel!

    init(
        payload: SharedPagePayload? = SharedPagePayload(
            html: "<html></html>",
            url: URL(string: "https://example.com/recipe")!,
            title: nil
        ),
        isLoggedIn: Bool = true
    ) {
        let clock = clock
        let gate = gate
        model = ShareCaptureModel(
            payload: payload,
            isLoggedIn: { isLoggedIn },
            capture: { _ in try await gate.capture() },
            sleep: { await clock.sleep($0) },
            onComplete: { [weak self] in self?.completions += 1 },
            onCancel: { [weak self] in self?.cancellations += 1 }
        )
    }
}

/// Let pending main-actor tasks run.
@MainActor
private func settle() async {
    for _ in 0..<20 {
        await Task.yield()
    }
}

@MainActor
private func waitUntil(
    _ condition: () -> Bool,
    file: StaticString = #filePath,
    line: UInt = #line
) async {
    for _ in 0..<500 {
        if condition() { return }
        await Task.yield()
        try? await Task.sleep(nanoseconds: 2_000_000)
    }
    XCTFail("Condition not met", file: file, line: line)
}
