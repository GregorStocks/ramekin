import Foundation
import UniformTypeIdentifiers
import XCTest
@testable import Ramekin

final class SharedPagePayloadExtractorTests: XCTestCase {
    func testExtractsPayloadFromPreprocessingResults() async throws {
        let results: [String: Any] = [
            "html": "<html><body>hi</body></html>",
            "url": "https://example.com/recipe",
            "title": "Example Recipe"
        ]
        let provider = FakePagePayloadItemProvider(
            typeIdentifier: UTType.propertyList.identifier,
            result: .success(payloadDictionary(with: results))
        )

        let payload = try await XCTUnwrapAsync(
            await SharedPagePayloadExtractor.extractPayload(from: [provider])
        )

        XCTAssertEqual(payload.html, "<html><body>hi</body></html>")
        XCTAssertEqual(payload.url, URL(string: "https://example.com/recipe"))
        XCTAssertEqual(payload.title, "Example Recipe")
    }

    func testReturnsNilWhenNoPropertyListProvider() async {
        let provider = FakePagePayloadItemProvider(
            typeIdentifier: UTType.url.identifier,
            result: .success(NSURL(string: "https://example.com")!)
        )

        let payload = await SharedPagePayloadExtractor.extractPayload(from: [provider])

        XCTAssertNil(payload)
    }

    func testReturnsNilWhenResultsKeyMissing() async {
        let provider = FakePagePayloadItemProvider(
            typeIdentifier: UTType.propertyList.identifier,
            result: .success(NSDictionary(dictionary: ["wrong": "thing"]))
        )

        let payload = await SharedPagePayloadExtractor.extractPayload(from: [provider])

        XCTAssertNil(payload)
    }

    func testReturnsNilWhenHtmlOrUrlMissing() async {
        let provider = FakePagePayloadItemProvider(
            typeIdentifier: UTType.propertyList.identifier,
            result: .success(payloadDictionary(with: ["html": "<p/>"])) // no url
        )

        let payload = await SharedPagePayloadExtractor.extractPayload(from: [provider])

        XCTAssertNil(payload)
    }

    func testReturnsNilWhenUrlUnparseable() async {
        let provider = FakePagePayloadItemProvider(
            typeIdentifier: UTType.propertyList.identifier,
            result: .success(payloadDictionary(with: [
                "html": "<p/>",
                "url": "" // empty string cannot build a URL
            ]))
        )

        let payload = await SharedPagePayloadExtractor.extractPayload(from: [provider])

        XCTAssertNil(payload)
    }

    func testTitleIsOptional() async throws {
        let provider = FakePagePayloadItemProvider(
            typeIdentifier: UTType.propertyList.identifier,
            result: .success(payloadDictionary(with: [
                "html": "<p/>",
                "url": "https://example.com/x"
            ]))
        )

        let payload = try await XCTUnwrapAsync(
            await SharedPagePayloadExtractor.extractPayload(from: [provider])
        )
        XCTAssertNil(payload.title)
    }

    // MARK: - Helpers

    private func payloadDictionary(with results: [String: Any]) -> NSDictionary {
        NSDictionary(dictionary: [
            NSExtensionJavaScriptPreprocessingResultsKey: results
        ])
    }

    private func XCTUnwrapAsync<T>(
        _ value: T?,
        file: StaticString = #file,
        line: UInt = #line
    ) async throws -> T {
        guard let value else {
            XCTFail("Expected non-nil value", file: file, line: line)
            throw XCTSkip("nil")
        }
        return value
    }
}

private final class FakePagePayloadItemProvider: SharedPagePayloadItemProvider {
    private let typeIdentifier: String
    private let result: Result<NSSecureCoding?, Error>

    init(typeIdentifier: String, result: Result<NSSecureCoding?, Error>) {
        self.typeIdentifier = typeIdentifier
        self.result = result
    }

    var registeredTypeIdentifiers: [String] { [typeIdentifier] }

    func hasItemConformingToTypeIdentifier(_ typeIdentifier: String) -> Bool {
        typeIdentifier == self.typeIdentifier
    }

    func loadItem(
        forTypeIdentifier typeIdentifier: String,
        completionHandler: @escaping @Sendable (NSSecureCoding?, Error?) -> Void
    ) {
        DispatchQueue.global().asyncAfter(deadline: .now() + 0.01) { [result] in
            switch result {
            case .success(let item):
                completionHandler(item, nil)
            case .failure(let error):
                completionHandler(nil, error)
            }
        }
    }
}
