import XCTest
@testable import Ramekin

final class RamekinAPITests: XCTestCase {

    // MARK: - URL Normalization Tests

    func testNormalizeURLAddsHTTPS() async throws {
        // Given a URL without protocol
        let input = "example.com"

        // When normalized
        let normalized = normalizeServerURL(input)

        // Then it should add https://
        XCTAssertEqual(normalized, "https://example.com")
    }

    func testNormalizeURLPreservesHTTPS() async throws {
        // Given a URL with https
        let input = "https://example.com"

        // When normalized
        let normalized = normalizeServerURL(input)

        // Then it should remain unchanged
        XCTAssertEqual(normalized, "https://example.com")
    }

    func testNormalizeURLPreservesHTTP() async throws {
        // Given a URL with http (local dev)
        let input = "http://localhost:3000"

        // When normalized
        let normalized = normalizeServerURL(input)

        // Then it should preserve http
        XCTAssertEqual(normalized, "http://localhost:3000")
    }

    func testNormalizeURLRemovesTrailingSlash() async throws {
        // Given a URL with trailing slash
        let input = "https://example.com/"

        // When normalized
        let normalized = normalizeServerURL(input)

        // Then trailing slash should be removed
        XCTAssertEqual(normalized, "https://example.com")
    }

    func testNormalizeURLTrimsWhitespace() async throws {
        // Given a URL with whitespace
        let input = "  example.com  "

        // When normalized
        let normalized = normalizeServerURL(input)

        // Then whitespace should be trimmed
        XCTAssertEqual(normalized, "https://example.com")
    }

    func testNormalizeURLComplexCase() async throws {
        // Given a URL with multiple issues
        let input = "  my-server.example.com/  "

        // When normalized
        let normalized = normalizeServerURL(input)

        // Then all issues should be fixed
        XCTAssertEqual(normalized, "https://my-server.example.com")
    }

    // MARK: - API Error Tests

    func testAPIErrorDescriptions() {
        // Test that all error cases have meaningful descriptions
        let errors: [RamekinAPI.APIError] = [
            .noServerURL,
            .noAuthToken,
            .invalidURL,
            .invalidResponse,
            .httpError(401, "Unauthorized"),
            .httpError(500, nil),
            .networkError(URLError(.notConnectedToInternet)),
            .decodingError(DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "test")))
        ]

        for error in errors {
            XCTAssertNotNil(error.errorDescription, "Error \(error) should have a description")
            XCTAssertFalse(error.errorDescription!.isEmpty, "Error \(error) description should not be empty")
        }
    }

    func testHTTPErrorWithMessage() {
        let error = RamekinAPI.APIError.httpError(401, "Invalid credentials")
        XCTAssertEqual(error.errorDescription, "Invalid credentials")
    }

    func testHTTPErrorWithoutMessage() {
        let error = RamekinAPI.APIError.httpError(500, nil)
        XCTAssertEqual(error.errorDescription, "HTTP error 500")
    }

    // MARK: - Request Encoding Tests

    func testLoginRequestEncoding() throws {
        let request = RamekinAPI.LoginRequest(username: "testuser", password: "testpass")
        let data = try JSONEncoder().encode(request)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: String] else {
            XCTFail("Failed to decode JSON as [String: String]")
            return
        }

        XCTAssertEqual(json["username"], "testuser")
        XCTAssertEqual(json["password"], "testpass")
    }

    func testScrapeRequestEncoding() throws {
        let request = RamekinAPI.ScrapeRequest(url: "https://example.com/recipe")
        let data = try JSONEncoder().encode(request)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: String] else {
            XCTFail("Failed to decode JSON as [String: String]")
            return
        }

        XCTAssertEqual(json["url"], "https://example.com/recipe")
    }

    // MARK: - Response Decoding Tests

    func testLoginResponseDecoding() throws {
        let json = """
        {"token": "abc123xyz"}
        """
        let data = json.data(using: .utf8)!
        let response = try JSONDecoder().decode(RamekinAPI.LoginResponse.self, from: data)

        XCTAssertEqual(response.token, "abc123xyz")
    }

    func testScrapeResponseDecoding() throws {
        let json = """
        {"id": "job-456"}
        """
        let data = json.data(using: .utf8)!
        let response = try JSONDecoder().decode(RamekinAPI.ScrapeResponse.self, from: data)

        XCTAssertEqual(response.id, "job-456")
    }

    func testScrapeJobStatusDecoding() throws {
        let json = """
        {
            "id": "job-789",
            "status": "completed",
            "recipe_id": "recipe-123",
            "error_message": null
        }
        """
        let data = json.data(using: .utf8)!
        let response = try JSONDecoder().decode(RamekinAPI.ScrapeJobStatus.self, from: data)

        XCTAssertEqual(response.id, "job-789")
        XCTAssertEqual(response.status, "completed")
        XCTAssertEqual(response.recipe_id, "recipe-123")
        XCTAssertNil(response.error_message)
    }

    func testScrapeJobStatusWithError() throws {
        let json = """
        {
            "id": "job-fail",
            "status": "failed",
            "recipe_id": null,
            "error_message": "Could not parse recipe"
        }
        """
        let data = json.data(using: .utf8)!
        let response = try JSONDecoder().decode(RamekinAPI.ScrapeJobStatus.self, from: data)

        XCTAssertEqual(response.id, "job-fail")
        XCTAssertEqual(response.status, "failed")
        XCTAssertNil(response.recipe_id)
        XCTAssertEqual(response.error_message, "Could not parse recipe")
    }

    func testErrorResponseDecoding() throws {
        let json1 = """
        {"error": "Something went wrong"}
        """
        let data1 = json1.data(using: .utf8)!
        let response1 = try JSONDecoder().decode(RamekinAPI.ErrorResponse.self, from: data1)
        XCTAssertEqual(response1.errorMessage, "Something went wrong")

        let json2 = """
        {"message": "Another error"}
        """
        let data2 = json2.data(using: .utf8)!
        let response2 = try JSONDecoder().decode(RamekinAPI.ErrorResponse.self, from: data2)
        XCTAssertEqual(response2.errorMessage, "Another error")
    }

    // MARK: - Helper

    /// Extracted URL normalization logic for testing
    private func normalizeServerURL(_ serverURL: String) -> String {
        var normalized = serverURL.trimmingCharacters(in: .whitespacesAndNewlines)
        if !normalized.hasPrefix("http://") && !normalized.hasPrefix("https://") {
            normalized = "https://\(normalized)"
        }
        if normalized.hasSuffix("/") {
            normalized = String(normalized.dropLast())
        }
        return normalized
    }
}
