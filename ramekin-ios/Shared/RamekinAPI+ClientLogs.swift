import Foundation
import UIKit

private struct UploadLogsRequestBody: Encodable {
    let platform: String
    let appVersion: String?
    let osInfo: String?
    let content: String

    enum CodingKeys: String, CodingKey {
        case platform
        case appVersion = "app_version"
        case osInfo = "os_info"
        case content
    }
}

extension RamekinAPI {
    /// Uploads the DebugLogger contents to the server for diagnostics.
    func uploadLogs(_ content: String) async throws {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String
        let build = info?["CFBundleVersion"] as? String
        let appVersion = [version, build].compactMap { $0 }.joined(separator: " ")

        let body = try JSONEncoder().encode(UploadLogsRequestBody(
            platform: "ios",
            appVersion: appVersion.isEmpty ? nil : appVersion,
            osInfo: "iOS \(UIDevice.current.systemVersion)",
            content: content
        ))
        // Uploading the log must not append the log to itself, so don't log
        // this request's body (it self-amplifies: repeat uploads would nest
        // prior uploads inside the log they're uploading).
        _ = try await performRequest(
            method: "POST",
            path: "/api/client-logs",
            body: body,
            acceptedStatusCodes: [201],
            logBody: false
        )
    }
}
