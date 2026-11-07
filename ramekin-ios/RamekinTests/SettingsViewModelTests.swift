import XCTest
@testable import Ramekin

@MainActor
final class SettingsViewModelTests: XCTestCase {
    func testOlderConnectionCheckCannotOverwriteNewerOne() async {
        let firstStarted = RequestGate()
        let releaseFirst = RequestGate()
        var calls = 0
        let model = makeModel(testConnection: {
            calls += 1
            if calls == 1 {
                await firstStarted.open()
                await releaseFirst.wait()
                throw URLError(.notConnectedToInternet)
            }
            return true
        })

        let first = Task { await model.checkConnection() }
        await firstStarted.wait()
        await model.checkConnection()
        XCTAssertEqual(model.connectionStatus, .connected)

        releaseFirst.open()
        await first.value

        XCTAssertEqual(model.connectionStatus, .connected)
    }

    func testCancelledConnectionCheckIsNotReportedOffline() async {
        let model = makeModel(testConnection: { throw URLError(.cancelled) })

        await model.checkConnection()

        XCTAssertEqual(model.connectionStatus, .unknown)
    }

    func testFailedConnectionCheckReportsOffline() async {
        let model = makeModel(testConnection: { throw URLError(.notConnectedToInternet) })

        await model.checkConnection()

        XCTAssertEqual(model.connectionStatus, .failed("Offline"))
    }

    func testLogUploadMovesThroughOneStateAtATime() async {
        let uploadStarted = RequestGate()
        let releaseUpload = RequestGate()
        var uploads = 0
        let model = makeModel(uploadLogs: { _ in
            uploads += 1
            await uploadStarted.open()
            await releaseUpload.wait()
        })

        let upload = Task { await model.uploadLogs() }
        await uploadStarted.wait()
        XCTAssertEqual(model.logUpload, .uploading)
        await model.uploadLogs()
        releaseUpload.open()
        await upload.value

        XCTAssertEqual(uploads, 1)
        XCTAssertEqual(model.logUpload, .succeeded)

        model.dismissLogUploadResult()
        XCTAssertEqual(model.logUpload, .idle)
    }

    func testEmptyLogsFailWithoutUploading() async {
        let model = makeModel(
            readLogs: { "" },
            uploadLogs: { _ in XCTFail("Nothing to upload") }
        )

        await model.uploadLogs()

        XCTAssertEqual(model.logUpload, .failed("No logs to upload"))
    }

    func testFailedUploadReportsError() async {
        let model = makeModel(uploadLogs: { _ in throw RamekinAPI.APIError.httpError(500, nil, "Boom") })

        await model.uploadLogs()

        XCTAssertEqual(model.logUpload, .failed("Boom"))
    }

    func testExportFailureShowsMessage() async {
        let model = makeModel(exportAllRecipes: { throw RamekinAPI.APIError.httpError(500, nil, "No export") })

        await model.exportAllRecipes()

        XCTAssertEqual(model.exportError, "No export")
        XCTAssertNil(model.exportShareItem)
        XCTAssertFalse(model.isExportingAll)
    }

    func testExportWritesShareableFile() async throws {
        let model = makeModel(exportAllRecipes: {
            RamekinAPI.ExportDownload(data: Data("zip".utf8), filename: "recipes.paprikarecipes")
        })

        await model.exportAllRecipes()

        let url = try XCTUnwrap(model.exportShareItem?.url)
        XCTAssertEqual(url.lastPathComponent, "recipes.paprikarecipes")
        XCTAssertNil(model.exportError)
        XCTAssertFalse(model.isExportingAll)
    }

    private func makeModel(
        testConnection: @escaping () async throws -> Bool = { true },
        exportAllRecipes: @escaping () async throws -> RamekinAPI.ExportDownload = {
            throw URLError(.badURL)
        },
        readLogs: @escaping () -> String = { "log line" },
        uploadLogs: @escaping (String) async throws -> Void = { _ in }
    ) -> SettingsViewModel {
        SettingsViewModel(client: SettingsViewModel.Client(
            testConnection: testConnection,
            exportAllRecipes: exportAllRecipes,
            readLogs: readLogs,
            uploadLogs: uploadLogs
        ))
    }
}
