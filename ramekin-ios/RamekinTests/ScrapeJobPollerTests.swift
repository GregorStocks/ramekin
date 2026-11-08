import XCTest
@testable import Ramekin

@MainActor
final class ScrapeJobPollerTests: XCTestCase {
    func testStopsWhenTheServerSaysTheJobIsTerminal() async throws {
        var responses = [
            job(.parsing),
            job(.normalizing, recipeReady: true),
            job(.completed, recipeReady: true)
        ]
        var updates: [ScrapeJobStatus] = []
        let poller = ScrapeJobPoller(getScrape: { _ in responses.removeFirst() }, intervalNanoseconds: 0)

        let result = try await poller.poll(UUID()) { updates.append($0.status) }

        guard case .completed = result else { return XCTFail("expected completed, got \(result)") }
        XCTAssertEqual(updates, [.parsing, .normalizing, .completed])
    }

    func testReturnsFailedWithTheJob() async throws {
        let poller = ScrapeJobPoller(getScrape: { _ in self.job(.failed, error: "boom") }, intervalNanoseconds: 0)

        let result = try await poller.poll(UUID())

        guard case .failed(let failed) = result else { return XCTFail("expected failed, got \(result)") }
        XCTAssertEqual(failed.error, "boom")
    }

    func testUntilRecipeReadyStopsOnceTheServerSaysSo() async throws {
        var responses = [job(.parsing), job(.normalizing, recipeReady: true)]
        let poller = ScrapeJobPoller(getScrape: { _ in responses.removeFirst() }, intervalNanoseconds: 0)

        let result = try await poller.poll(UUID(), untilRecipeReady: true)

        guard case .recipeReady = result else { return XCTFail("expected recipeReady, got \(result)") }
        XCTAssertTrue(responses.isEmpty)
    }

    func testRetriesTransientErrorsThenGivesUp() async {
        var calls = 0
        let poller = ScrapeJobPoller(
            getScrape: { _ in
                calls += 1
                throw URLError(.networkConnectionLost)
            },
            intervalNanoseconds: 0,
            maxConsecutiveErrors: 3
        )

        do {
            _ = try await poller.poll(UUID())
            XCTFail("expected the poll to give up")
        } catch {
            XCTAssertEqual(calls, 3)
        }
    }

    func testAnErrorBetweenGoodPollsIsRetried() async throws {
        var calls = 0
        let poller = ScrapeJobPoller(
            getScrape: { _ in
                calls += 1
                if calls == 2 { throw URLError(.timedOut) }
                return self.job(calls == 1 ? .parsing : .completed, recipeReady: calls > 1)
            },
            intervalNanoseconds: 0
        )

        let result = try await poller.poll(UUID())

        guard case .completed = result else { return XCTFail("expected completed, got \(result)") }
        XCTAssertEqual(calls, 3)
    }

    private func job(_ status: ScrapeJobStatus, recipeReady: Bool = false, error: String? = nil) -> ScrapeJobResponse {
        ScrapeJobResponse(
            canRetry: status == .failed,
            createdAt: Date(timeIntervalSince1970: 1),
            error: error,
            id: UUID(),
            isTerminal: status == .completed || status == .failed,
            recipeId: recipeReady ? UUID() : nil,
            recipeReady: recipeReady,
            retryCount: 0,
            status: status,
            steps: []
        )
    }
}
