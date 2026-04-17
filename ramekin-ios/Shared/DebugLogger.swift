import Foundation

/// Debug logger that writes to a shared file in the app group container.
/// This allows both the main app and share extension to read/write logs.
class DebugLogger {
    static let shared = DebugLogger()

    private static let logFileName = "debug.log"
    private static let rotatedLogFileName = "debug.log.1"
    private static let defaultMaxLogSizeBytes = 2 * 1024 * 1024

    private let fileManager = FileManager.default
    private let queue = DispatchQueue(label: "com.ramekin.debuglogger", qos: .utility)
    private let dateFormatter: DateFormatter
    private let logDirectoryURLProvider: () -> URL?
    private let maxLogSizeBytes: Int
    private var fileHandle: FileHandle?
    private var fileHandleFileID: UInt64?

    private var logFileURL: URL? {
        logDirectoryURLProvider()?.appendingPathComponent(Self.logFileName)
    }

    private var rotatedLogFileURL: URL? {
        logDirectoryURLProvider()?.appendingPathComponent(Self.rotatedLogFileName)
    }

    private init() {
        logDirectoryURLProvider = {
            FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.com.ramekin.app")
        }
        maxLogSizeBytes = Self.defaultMaxLogSizeBytes
        dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
    }

    init(logDirectoryURL: URL, maxLogSizeBytes: Int = 2 * 1024 * 1024) {
        logDirectoryURLProvider = { logDirectoryURL }
        self.maxLogSizeBytes = maxLogSizeBytes
        dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
    }

    func log(_ message: String, source: String? = nil) {
        queue.async { [weak self] in
            guard let self = self else { return }

            let timestamp = self.dateFormatter.string(from: Date())
            let prefix = source.map { "[\($0)] " } ?? ""
            let entry = "[\(timestamp)] \(prefix)\(message)\n"
            guard let data = entry.data(using: .utf8) else { return }
            self.writeEntry(data)
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
        queue.sync {
            guard let url = logFileURL else { return "" }
            return (try? String(contentsOf: url, encoding: .utf8)) ?? ""
        }
    }

    func clearLogs() {
        queue.sync {
            closeFileHandle()
            if let url = logFileURL {
                try? fileManager.removeItem(at: url)
            }
            if let url = rotatedLogFileURL {
                try? fileManager.removeItem(at: url)
            }
        }
    }

    private func writeEntry(_ data: Data) {
        guard let logFileURL else { return }
        try? fileManager.createDirectory(
            at: logFileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true,
            attributes: nil
        )
        rotateLogsIfNeeded(forAppending: data.count)

        guard let handle = openFileHandle() else { return }
        try? handle.seekToEnd()
        try? handle.write(contentsOf: data)
    }

    private func rotateLogsIfNeeded(forAppending dataSize: Int) {
        guard let logFileURL else { return }
        let currentSize = (try? fileManager.attributesOfItem(atPath: logFileURL.path)[.size] as? NSNumber)?.intValue ?? 0
        guard currentSize + dataSize > maxLogSizeBytes else { return }

        closeFileHandle()
        if let rotatedLogFileURL {
            try? fileManager.removeItem(at: rotatedLogFileURL)
            if fileManager.fileExists(atPath: logFileURL.path) {
                try? fileManager.moveItem(at: logFileURL, to: rotatedLogFileURL)
            }
        } else {
            try? fileManager.removeItem(at: logFileURL)
        }
    }

    private func openFileHandle() -> FileHandle? {
        let currentFileID = currentLogFileID()
        if let fileHandle, let currentFileID, fileHandleFileID == currentFileID {
            return fileHandle
        }
        closeFileHandle()
        guard let logFileURL else { return nil }
        if currentFileID == nil {
            _ = fileManager.createFile(atPath: logFileURL.path, contents: nil)
        }
        guard let handle = try? FileHandle(forWritingTo: logFileURL) else { return nil }
        fileHandle = handle
        fileHandleFileID = currentLogFileID()
        return handle
    }

    private func closeFileHandle() {
        try? fileHandle?.close()
        fileHandle = nil
        fileHandleFileID = nil
    }

    private func currentLogFileID() -> UInt64? {
        guard let logFileURL else { return nil }
        return (try? fileManager.attributesOfItem(atPath: logFileURL.path)[.systemFileNumber] as? NSNumber)?.uint64Value
    }
}
