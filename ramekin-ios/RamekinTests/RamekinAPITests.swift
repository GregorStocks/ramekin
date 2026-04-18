import XCTest
@testable import Ramekin

final class RamekinAPITests: XCTestCase {

    // MARK: - URL Normalization Tests

    func testNormalizeURLAddsHTTPS() async throws {
        // Given a URL without protocol
        let input = "example.com"

        // When normalized
        let normalized = RamekinAPI.normalizeServerURL(input)

        // Then it should add https://
        XCTAssertEqual(normalized, "https://example.com")
    }

    func testNormalizeURLPreservesHTTPS() async throws {
        // Given a URL with https
        let input = "https://example.com"

        // When normalized
        let normalized = RamekinAPI.normalizeServerURL(input)

        // Then it should remain unchanged
        XCTAssertEqual(normalized, "https://example.com")
    }

    func testNormalizeURLPreservesHTTP() async throws {
        // Given a URL with http (local dev)
        let input = "http://localhost:3000"

        // When normalized
        let normalized = RamekinAPI.normalizeServerURL(input)

        // Then it should preserve http
        XCTAssertEqual(normalized, "http://localhost:3000")
    }

    func testNormalizeURLRemovesTrailingSlash() async throws {
        // Given a URL with trailing slash
        let input = "https://example.com/"

        // When normalized
        let normalized = RamekinAPI.normalizeServerURL(input)

        // Then trailing slash should be removed
        XCTAssertEqual(normalized, "https://example.com")
    }

    func testNormalizeURLTrimsWhitespace() async throws {
        // Given a URL with whitespace
        let input = "  example.com  "

        // When normalized
        let normalized = RamekinAPI.normalizeServerURL(input)

        // Then whitespace should be trimmed
        XCTAssertEqual(normalized, "https://example.com")
    }

    func testNormalizeURLComplexCase() async throws {
        // Given a URL with multiple issues
        let input = "  my-server.example.com/  "

        // When normalized
        let normalized = RamekinAPI.normalizeServerURL(input)

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

    func testInsecureSessionUsesInsecureSessionDelegate() {
        XCTAssertTrue(insecureSession.delegate is InsecureSessionDelegate)
    }

    func testLoginPersistsNormalizedURLTokenAndUsername() async throws {
        let credentialStore = MockCredentialStore()
        let api = RamekinAPI.shared
        let responseData = Data(#"{"token":"secret-token"}"#.utf8)

        let token = try await api.login(
            serverURL: " example.com/ ",
            username: "gregor",
            password: "pw",
            accessClientId: "cf-id",
            accessClientSecret: "cf-secret",
            credentialStore: credentialStore,
            updateClientConfiguration: false,
            requestExecutor: { request in
                XCTAssertEqual(request.url?.absoluteString, "https://example.com/api/auth/login")
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.value(forHTTPHeaderField: "CF-Access-Client-Id"), "cf-id")
                XCTAssertEqual(
                    request.value(forHTTPHeaderField: "CF-Access-Client-Secret"),
                    "cf-secret"
                )
                let response = HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: nil
                )!
                return (responseData, response)
            }
        )

        XCTAssertEqual(token, "secret-token")
        XCTAssertEqual(credentialStore.serverURL, "https://example.com")
        XCTAssertEqual(credentialStore.token, "secret-token")
        XCTAssertEqual(credentialStore.username, "gregor")
        XCTAssertEqual(credentialStore.accessClientId, "cf-id")
        XCTAssertEqual(credentialStore.accessClientSecret, "cf-secret")
    }

    func testLoginClearsStaleAccessCredentialsWhenValuesAreBlank() async {
        let credentialStore = MockCredentialStore()
        credentialStore.accessClientId = "stale-id"
        credentialStore.accessClientSecret = "stale-secret"

        await XCTAssertThrowsErrorAsync {
            try await RamekinAPI.shared.login(
                serverURL: "example.com",
                username: "gregor",
                password: "pw",
                accessClientId: " ",
                accessClientSecret: "",
                credentialStore: credentialStore,
                updateClientConfiguration: false,
                requestExecutor: { _ in
                    throw URLError(.notConnectedToInternet)
                }
            )
        }

        XCTAssertNil(credentialStore.accessClientId)
        XCTAssertNil(credentialStore.accessClientSecret)
    }

    func testScrapeSubmitTimeoutFitsShareExtensionBudget() {
        // iOS terminates share extensions around ~30s; the submit call must
        // return well before that so the user sees a result instead of the
        // OS killing the extension mid-spinner.
        XCTAssertLessThanOrEqual(RamekinAPI.scrapeSubmitTimeout, 20)
        XCTAssertGreaterThan(RamekinAPI.scrapeSubmitTimeout, 0)
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

    func testCaptureRequestEncoding() throws {
        let request = RamekinAPI.CaptureRequest(
            html: "<html><body>hi</body></html>",
            source_url: "https://example.com/recipe"
        )
        let data = try JSONEncoder().encode(request)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: String] else {
            XCTFail("Failed to decode JSON as [String: String]")
            return
        }

        XCTAssertEqual(json["html"], "<html><body>hi</body></html>")
        XCTAssertEqual(json["source_url"], "https://example.com/recipe")
    }

    func testCreateMealPlanRequestEncodingOmitsEmptyNotes() throws {
        let recipeId = UUID(uuidString: "12345678-1234-1234-1234-123456789ABC")!
        let request = CreateMealPlanRequestBody(
            recipeId: recipeId,
            mealDate: "2026-04-17",
            mealType: "dinner",
            notes: nil
        )
        let data = try JSONEncoder().encode(request)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: String] else {
            XCTFail("Failed to decode JSON as [String: String]")
            return
        }

        XCTAssertEqual(json["recipe_id"], recipeId.uuidString)
        XCTAssertEqual(json["meal_date"], "2026-04-17")
        XCTAssertEqual(json["meal_type"], "dinner")
        XCTAssertNil(json["notes"])
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
}

private final class MockCredentialStore: CredentialStore {
    var token: String?
    var serverURL: String?
    var username: String?
    var accessClientId: String?
    var accessClientSecret: String?

    func saveToken(_ token: String) -> Bool {
        self.token = token
        return true
    }

    func getToken() -> String? { token }

    func deleteToken() {
        token = nil
    }

    func saveServerURL(_ url: String) -> Bool {
        serverURL = url
        return true
    }

    func getServerURL() -> String? { serverURL }

    func saveUsername(_ username: String) -> Bool {
        self.username = username
        return true
    }

    func getUsername() -> String? { username }

    func saveAccessClientId(_ value: String) -> Bool {
        accessClientId = value
        return true
    }

    func getAccessClientId() -> String? { accessClientId }

    func deleteAccessClientId() {
        accessClientId = nil
    }

    func saveAccessClientSecret(_ value: String) -> Bool {
        accessClientSecret = value
        return true
    }

    func getAccessClientSecret() -> String? { accessClientSecret }

    func deleteAccessClientSecret() {
        accessClientSecret = nil
    }

    func clearAll() {
        token = nil
        serverURL = nil
        username = nil
        accessClientId = nil
        accessClientSecret = nil
    }
}

private func XCTAssertThrowsErrorAsync<T>(
    _ expression: () async throws -> T,
    file: StaticString = #filePath,
    line: UInt = #line
) async {
    do {
        _ = try await expression()
        XCTFail("Expected expression to throw", file: file, line: line)
    } catch {
    }
}
