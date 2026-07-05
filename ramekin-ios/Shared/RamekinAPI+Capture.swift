import Foundation

// MARK: - Scraping / capture

extension RamekinAPI {
    /// Timeout for the cheap auth pre-flight (`GET /api/users/me`) run before a
    /// capture upload. The pre-flight and the upload run sequentially in the
    /// share extension, so this plus `captureSubmitTimeout` must stay under the
    /// ~30s iOS share-extension budget (5 + 23 = 28). Short, since `/me` is a
    /// tiny request and a real stall should fail fast.
    static let authCheckTimeout: TimeInterval = 5

    func captureHTML(html: String, sourceURL: String) async throws -> CreateScrapeResponse {
        // `RamekinAPI.logger` is file-private, so this cross-file extension uses
        // the shared logger directly (it is the same instance).
        DebugLogger.shared.log("captureHTML called for: \(sourceURL) (html \(html.count) bytes)")
        let response = try await executeGenerated(timeoutInterval: Self.captureSubmitTimeout) {
            try await ScrapeAPI.capture(
                captureRequest: CaptureRequest(html: html, sourceUrl: sourceURL)
            )
        }
        DebugLogger.shared.log("SUCCESS: Capture job ID: \(response.id.uuidString)")
        return response
    }

    /// Check the status of a scrape job
    func getScrapeStatus(id: String) async throws -> ScrapeJobResponse {
        guard let uuid = UUID(uuidString: id) else {
            throw APIError.invalidURL
        }
        return try await executeGenerated {
            try await ScrapeAPI.getScrape(id: uuid)
        }
    }

    /// Verify the stored token is still valid before doing expensive work like
    /// uploading a captured page. Throws `APIError.httpError(401/403, …)` (or
    /// `.noAuthToken`) when the session is expired or not permitted, so callers
    /// can show a clear "sign in again" message instead of hanging on a doomed
    /// upload.
    func verifyAuthenticated() async throws {
        _ = try await executeGenerated(timeoutInterval: Self.authCheckTimeout) {
            try await UsersAPI.me()
        }
    }
}
