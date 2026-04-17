import Foundation

enum SearchDebounceSupport {
    static let delayNanoseconds: UInt64 = 300_000_000

    @MainActor
    static func replaceTask(
        _ searchTask: inout Task<Void, Never>?,
        operation: @escaping @Sendable () async -> Void
    ) {
        cancelTask(&searchTask)
        searchTask = Task {
            do {
                try await Task.sleep(nanoseconds: delayNanoseconds)
            } catch is CancellationError {
                return
            } catch {
                return
            }

            guard !Task.isCancelled else {
                return
            }

            await operation()
        }
    }

    @MainActor
    static func cancelTask(_ searchTask: inout Task<Void, Never>?) {
        searchTask?.cancel()
        searchTask = nil
    }
}
