import Foundation

// MARK: - Scraping / capture

extension RamekinAPI {
    /// Timeout for the cheap auth pre-flight (`GET /api/users/me`) run before a
    /// capture upload. The pre-flight and the upload run sequentially in the
    /// share extension, so this plus `captureSubmitTimeout` must stay under the
    /// ~30s iOS share-extension budget (5 + 23 = 28). Short, since `/me` is a
    /// tiny request and a real stall should fail fast.
    static let authCheckTimeout: TimeInterval = 5

    func captureHTML(html: String, sourceURL: String) async throws -> ScrapeResponse {
        // `RamekinAPI.logger` is file-private, so this cross-file extension uses
        // the shared logger directly (it is the same instance).
        DebugLogger.shared.log("captureHTML called for: \(sourceURL) (html \(html.count) bytes)")
        let body = try JSONEncoder().encode(CaptureRequest(html: html, source_url: sourceURL))
        let data = try await performRequest(
            method: "POST",
            path: "/api/scrape/capture",
            body: body,
            acceptedStatusCodes: [200, 201],
            timeoutInterval: Self.captureSubmitTimeout,
            logBody: false
        )
        let decoded = try JSONDecoder().decode(ScrapeResponse.self, from: data)
        DebugLogger.shared.log("SUCCESS: Capture job ID: \(decoded.id)")
        return decoded
    }

    /// Check the status of a scrape job
    func getScrapeStatus(id: String) async throws -> ScrapeJobStatus {
        let data = try await performRequest(
            method: "GET",
            path: "/api/scrape/\(id)",
            acceptedStatusCodes: [200]
        )
        return try JSONDecoder().decode(ScrapeJobStatus.self, from: data)
    }

    /// Verify the stored token is still valid before doing expensive work like
    /// uploading a captured page. Throws `APIError.httpError(401/403, …)` (or
    /// `.noAuthToken`) when the session is expired or not permitted, so callers
    /// can show a clear "sign in again" message instead of hanging on a doomed
    /// upload.
    func verifyAuthenticated() async throws {
        _ = try await performRequest(
            method: "GET",
            path: "/api/users/me",
            acceptedStatusCodes: [200],
            timeoutInterval: Self.authCheckTimeout
        )
    }
}
