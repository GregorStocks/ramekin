import Foundation

enum CancellationSupport {
    /// Whether `error` means the work was cancelled rather than failed.
    /// URLSession reports a cancelled request as `URLError.cancelled`, not a
    /// `CancellationError`, and the API layer wraps both, so a bare
    /// `catch is CancellationError` misses most real cancellations.
    static func isCancellation(_ error: Error) -> Bool {
        if error is CancellationError {
            return true
        }
        if let urlError = error as? URLError {
            return urlError.code == .cancelled
        }
        if case let RamekinAPI.APIError.networkError(underlying) = error {
            return isCancellation(underlying)
        }
        if case let ErrorResponse.error(_, _, _, underlying) = error {
            return isCancellation(underlying)
        }
        let nsError = error as NSError
        return nsError.domain == NSURLErrorDomain && nsError.code == NSURLErrorCancelled
    }

    /// Waits for `task` to finish, but returns as soon as the current task is
    /// cancelled. A plain `await task.value` ignores the waiter's
    /// cancellation. `task` keeps running either way.
    static func wait(for task: Task<Void, Never>) async {
        let resumer = ResumeOnce()
        await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                resumer.install(continuation)
                Task {
                    await task.value
                    resumer.resume()
                }
            }
        } onCancel: {
            resumer.resume()
        }
    }
}

/// Resumes a continuation exactly once, from whichever side gets there
/// first, including a cancellation that fires before it is installed.
private final class ResumeOnce: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Void, Never>?
    private var resumed = false

    func install(_ continuation: CheckedContinuation<Void, Never>) {
        lock.lock()
        if resumed {
            lock.unlock()
            continuation.resume()
            return
        }
        self.continuation = continuation
        lock.unlock()
    }

    func resume() {
        lock.lock()
        let pending = continuation
        continuation = nil
        resumed = true
        lock.unlock()
        pending?.resume()
    }
}
