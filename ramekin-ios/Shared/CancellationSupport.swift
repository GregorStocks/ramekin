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
}
