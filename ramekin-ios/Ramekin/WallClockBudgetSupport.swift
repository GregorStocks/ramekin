import Foundation

/// The operation given to `withWallClockBudget` did not finish in time.
struct WallClockBudgetExceeded: Error, LocalizedError {
    var errorDescription: String? { "The operation did not finish within its time budget" }
}

/// Races `operation` against a wall-clock budget, cancelling it and throwing
/// `WallClockBudgetExceeded` when the budget runs out first.
func withWallClockBudget(
    seconds: TimeInterval,
    _ operation: @escaping @MainActor () async throws -> Void
) async throws {
    try await withThrowingTaskGroup(of: Bool.self) { group in
        group.addTask {
            try await operation()
            return true
        }
        group.addTask {
            try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
            return false
        }
        defer { group.cancelAll() }
        // next() is never nil with two tasks added; treat it as a timeout anyway.
        guard let operationFinishedFirst = try await group.next(), operationFinishedFirst else {
            throw WallClockBudgetExceeded()
        }
    }
}
