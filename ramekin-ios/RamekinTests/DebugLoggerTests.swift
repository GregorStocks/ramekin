import Foundation
import XCTest
@testable import Ramekin

final class DebugLoggerTests: XCTestCase {
    private var tempDirectoryURL: URL!

    override func setUpWithError() throws {
        tempDirectoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(
            at: tempDirectoryURL,
            withIntermediateDirectories: true,
            attributes: nil
        )
    }

    override func tearDownWithError() throws {
        if let tempDirectoryURL {
            try? FileManager.default.removeItem(at: tempDirectoryURL)
        }
    }

    func testLoggerRotatesWhenAppendingPastSizeLimit() throws {
        let logger = DebugLogger(logDirectoryURL: tempDirectoryURL, maxLogSizeBytes: 80)

        logger.log(String(repeating: "A", count: 70), source: "First")
        XCTAssertFalse(logger.readLogs().isEmpty)

        logger.log(String(repeating: "B", count: 70), source: "Second")
        let currentLogs = logger.readLogs()

        XCTAssertTrue(currentLogs.contains("[Second]"))
        XCTAssertFalse(currentLogs.contains("[First]"))

        let rotatedLogs = try String(
            contentsOf: tempDirectoryURL.appendingPathComponent("debug.log.1"),
            encoding: .utf8
        )
        XCTAssertTrue(rotatedLogs.contains("[First]"))
        XCTAssertFalse(rotatedLogs.contains("[Second]"))
    }

    func testClearLogsRemovesCurrentAndRotatedFiles() {
        let logger = DebugLogger(logDirectoryURL: tempDirectoryURL, maxLogSizeBytes: 80)

        logger.log(String(repeating: "A", count: 70), source: "First")
        _ = logger.readLogs()
        logger.log(String(repeating: "B", count: 70), source: "Second")
        XCTAssertFalse(logger.readLogs().isEmpty)

        logger.clearLogs()

        XCTAssertEqual(logger.readLogs(), "")
        XCTAssertFalse(FileManager.default.fileExists(atPath: tempDirectoryURL.appendingPathComponent("debug.log").path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: tempDirectoryURL.appendingPathComponent("debug.log.1").path))
    }
}
