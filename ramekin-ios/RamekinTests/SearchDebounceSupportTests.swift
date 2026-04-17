import XCTest
@testable import Ramekin

final class SearchDebounceSupportTests: XCTestCase {
    @MainActor
    func testCancelTaskCancelsAndClearsOutstandingTask() async throws {
        var searchTask: Task<Void, Never>? = Task {
            try? await Task.sleep(nanoseconds: 5_000_000_000)
        }
        let outstandingTask = try XCTUnwrap(searchTask)

        SearchDebounceSupport.cancelTask(&searchTask)

        XCTAssertTrue(outstandingTask.isCancelled)
        XCTAssertNil(searchTask)
    }

    @MainActor
    func testReplaceTaskCancelsPreviousTaskAndRunsNewOperationAfterDelay() async throws {
        let recorder = DebounceRecorder()
        var searchTask: Task<Void, Never>? = Task {
            try? await Task.sleep(nanoseconds: 5_000_000_000)
        }
        let previousTask = try XCTUnwrap(searchTask)

        SearchDebounceSupport.replaceTask(&searchTask) {
            await recorder.recordRun()
        }

        XCTAssertTrue(previousTask.isCancelled)
        XCTAssertNotNil(searchTask)

        try await Task.sleep(nanoseconds: SearchDebounceSupport.delayNanoseconds + 50_000_000)

        let runCount = await recorder.runCount()
        XCTAssertEqual(runCount, 1)
    }

    @MainActor
    func testCancelTaskPreventsScheduledOperationFromRunning() async throws {
        let recorder = DebounceRecorder()
        var searchTask: Task<Void, Never>?

        SearchDebounceSupport.replaceTask(&searchTask) {
            await recorder.recordRun()
        }
        SearchDebounceSupport.cancelTask(&searchTask)

        try await Task.sleep(nanoseconds: SearchDebounceSupport.delayNanoseconds + 50_000_000)

        let runCount = await recorder.runCount()
        XCTAssertEqual(runCount, 0)
        XCTAssertNil(searchTask)
    }
}

private actor DebounceRecorder {
    private var count = 0

    func recordRun() {
        count += 1
    }

    func runCount() -> Int {
        count
    }
}
