import Foundation

/// Where the debug-log upload stands. The success and failure states are
/// shown as alerts until dismissed.
enum LogUploadState: Equatable {
    case idle
    case uploading
    case succeeded
    case failed(String)
}

enum ConnectionStatus: Equatable {
    case unknown
    case checking
    case connected
    case failed(String)
}

/// Settings actions that talk to the server: the connection check, the
/// all-recipes export, and the debug-log upload.
@MainActor
final class SettingsViewModel: ObservableObject {
    struct Client {
        var testConnection: () async throws -> Bool
        var exportAllRecipes: () async throws -> RamekinAPI.ExportDownload
        var readLogs: () -> String
        var uploadLogs: (String) async throws -> Void

        static let live = Client(
            testConnection: { try await RamekinAPI.shared.testConnection() },
            exportAllRecipes: { try await RamekinAPI.shared.exportAllRecipes() },
            readLogs: { DebugLogger.shared.readLogs() },
            uploadLogs: { try await RamekinAPI.shared.uploadLogs($0) }
        )
    }

    @Published private(set) var connectionStatus: ConnectionStatus = .unknown
    @Published private(set) var isExportingAll = false
    @Published var exportShareItem: ShareItem?
    @Published var exportError: String?
    @Published var logUpload: LogUploadState = .idle

    private let client: Client
    private var connectionCheck = UUID()

    init(client: Client = .live) {
        self.client = client
    }

    /// Checks the server is reachable. A newer check (pull-to-refresh while
    /// the on-appear one runs) supersedes an older one, and a cancelled
    /// check (the screen went away) reports nothing.
    func checkConnection() async {
        let check = UUID()
        connectionCheck = check
        connectionStatus = .checking
        let status: ConnectionStatus
        do {
            status = try await client.testConnection() ? .connected : .failed("Not reachable")
        } catch {
            guard connectionCheck == check else { return }
            if CancellationSupport.isCancellation(error) || Task.isCancelled {
                connectionStatus = .unknown
                return
            }
            status = .failed("Offline")
        }
        guard connectionCheck == check else { return }
        connectionStatus = Task.isCancelled ? .unknown : status
    }

    func exportAllRecipes() async {
        guard !isExportingAll else { return }
        isExportingAll = true
        defer { isExportingAll = false }
        switch await ExportOutcome.download(client.exportAllRecipes) {
        case let .ready(item): exportShareItem = item
        case let .failed(message): exportError = message
        case .cancelled: break
        }
    }

    func uploadLogs() async {
        guard logUpload != .uploading else { return }
        let content = client.readLogs()
        guard !content.isEmpty else {
            logUpload = .failed("No logs to upload")
            return
        }
        logUpload = .uploading
        do {
            try await client.uploadLogs(content)
            logUpload = .succeeded
        } catch let apiError as RamekinAPI.APIError {
            logUpload = .failed(apiError.errorDescription ?? "Upload failed")
        } catch {
            logUpload = .failed(error.localizedDescription)
        }
    }

    /// Dismisses the upload result alert.
    func dismissLogUploadResult() {
        if logUpload != .uploading {
            logUpload = .idle
        }
    }
}
