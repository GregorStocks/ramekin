import Foundation
import UIKit

extension RamekinAPI {
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
        _ = try await performRequest(
            method: "POST",
            path: "/api/client-logs",
            body: body,
            acceptedStatusCodes: [201]
        )
    }
}
