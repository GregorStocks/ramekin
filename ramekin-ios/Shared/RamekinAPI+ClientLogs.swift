import Foundation
import UIKit

extension RamekinAPI {
    /// Uploads the DebugLogger contents to the server for diagnostics.
    func uploadLogs(_ content: String) async throws {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String
        let build = info?["CFBundleVersion"] as? String
        let appVersion = [version, build].compactMap { $0 }.joined(separator: " ")

        _ = try await executeGenerated {
            try await ClientLogsAPI.createClientLog(createClientLogRequest: CreateClientLogRequest(
                appVersion: appVersion.isEmpty ? nil : appVersion,
                content: content,
                osInfo: "iOS \(UIDevice.current.systemVersion)",
                platform: "ios"
            ))
        }
    }
}
