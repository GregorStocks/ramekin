import Foundation

/// Debug logger that writes to a shared file in the app group container.
/// This allows both the main app and share extension to read/write logs.
class DebugLogger {
    static let shared = DebugLogger()

    private let fileManager = FileManager.default
    private let queue = DispatchQueue(label: "com.ramekin.debuglogger", qos: .utility)
    private let dateFormatter: DateFormatter

    private var logFileURL: URL? {
        guard let containerURL = fileManager.containerURL(forSecurityApplicationGroupIdentifier: "group.com.ramekin.app") else {
            return nil
        }
        return containerURL.appendingPathComponent("debug.log")
    }

    private init() {
        dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
    }

    func log(_ message: String, source: String? = nil) {
        queue.async { [weak self] in
            guard let self = self, let url = self.logFileURL else { return }

            let timestamp = self.dateFormatter.string(from: Date())
            let prefix = source.map { "[\($0)] " } ?? ""
            let entry = "[\(timestamp)] \(prefix)\(message)\n"

            if self.fileManager.fileExists(atPath: url.path) {
                if let handle = try? FileHandle(forWritingTo: url) {
                    handle.seekToEndOfFile()
                    if let data = entry.data(using: .utf8) {
                        handle.write(data)
                    }
                    try? handle.close()
                }
            } else {
                try? entry.write(to: url, atomically: true, encoding: .utf8)
            }
        }
    }

    func timed<T>(_ label: String, source: String? = nil, operation: () async throws -> T) async rethrows -> T {
        log("\(label) started", source: source)
        let start = CFAbsoluteTimeGetCurrent()
        do {
            let result = try await operation()
            let elapsed = CFAbsoluteTimeGetCurrent() - start
            log("\(label) completed (\(String(format: "%.2f", elapsed))s)", source: source)
            return result
        } catch {
            let elapsed = CFAbsoluteTimeGetCurrent() - start
            log("\(label) FAILED after \(String(format: "%.2f", elapsed))s: \(error.localizedDescription)", source: source)
            throw error
        }
    }

    func readLogs() -> String {
        guard let url = logFileURL else { return "" }
        return (try? String(contentsOf: url, encoding: .utf8)) ?? ""
    }

    func clearLogs() {
        guard let url = logFileURL else { return }
        try? fileManager.removeItem(at: url)
    }
}
