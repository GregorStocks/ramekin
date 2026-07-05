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
            .httpError(401, .unauthorized, "Unauthorized"),
            .httpError(500, nil, nil),
            .networkError(URLError(.notConnectedToInternet)),
            .decodingError(DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "test")))
        ]

        for error in errors {
            XCTAssertNotNil(error.errorDescription, "Error \(error) should have a description")
            XCTAssertFalse(error.errorDescription!.isEmpty, "Error \(error) description should not be empty")
        }
    }

    func testHTTPErrorWithMessage() {
        let error = RamekinAPI.APIError.httpError(401, .unauthorized, "Invalid credentials")
        XCTAssertEqual(error.errorDescription, "Invalid credentials")
        XCTAssertEqual(error.code, .unauthorized)
    }

    func testHTTPErrorWithoutMessage() {
        let error = RamekinAPI.APIError.httpError(500, nil, nil)
        XCTAssertEqual(error.errorDescription, "HTTP error 500")
        XCTAssertNil(error.code)
    }

    func testInsecureSessionUsesInsecureSessionDelegate() {
        XCTAssertTrue(insecureSession.delegate is InsecureSessionDelegate)
    }

    func testLoginPersistsNormalizedURLTokenAndUsername() async throws {
        let credentialStore = MockCredentialStore()
        let api = RamekinAPI.shared

        let token = try await api.login(
            serverURL: " example.com/ ",
            username: "gregor",
            password: "pw",
            accessClientId: "cf-id",
            accessClientSecret: "cf-secret",
            credentialStore: credentialStore,
            updateClientConfiguration: false,
            requestExecutor: { builder in
                XCTAssertEqual(builder.URLString, "https://example.com/api/auth/login")
                XCTAssertEqual(builder.method, "POST")
                XCTAssertEqual(builder.headers["CF-Access-Client-Id"], "cf-id")
                XCTAssertEqual(builder.headers["CF-Access-Client-Secret"], "cf-secret")

                let json = try requestBodyJSON(from: builder)
                XCTAssertEqual(json["username"] as? String, "gregor")
                XCTAssertEqual(json["password"] as? String, "pw")

                return LoginResponse(token: "secret-token")
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

    func testCaptureSubmitTimeoutFitsShareExtensionBudget() {
        // iOS terminates share extensions around ~30s; the submit call must
        // return before that so the user sees a result instead of the OS
        // killing the extension mid-spinner.
        XCTAssertLessThan(RamekinAPI.captureSubmitTimeout, 30)
        XCTAssertGreaterThan(RamekinAPI.captureSubmitTimeout, 0)
    }

    // MARK: - Request Encoding Tests

    func testLoginRequestEncoding() throws {
        let request = LoginRequest(password: "testpass", username: "testuser")
        let data = try JSONEncoder().encode(request)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: String] else {
            XCTFail("Failed to decode JSON as [String: String]")
            return
        }

        XCTAssertEqual(json["username"], "testuser")
        XCTAssertEqual(json["password"], "testpass")
    }

    func testCaptureRequestEncoding() throws {
        let request = CaptureRequest(
            html: "<html><body>hi</body></html>",
            sourceUrl: "https://example.com/recipe"
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
        let date = try XCTUnwrap(SharedDateFormatters.localDateOnly.date(from: "2026-04-17"))

        let builder = RamekinAPI.shared.buildMealPlanRequest {
            MealPlansAPI.createMealPlanWithRequestBuilder(createMealPlanRequest: CreateMealPlanRequest(
                mealDate: date,
                mealType: .dinner,
                notes: nil,
                recipeId: recipeId
            ))
        }
        let json = try requestBodyJSON(from: builder)

        XCTAssertEqual(json["recipe_id"] as? String, recipeId.uuidString)
        XCTAssertEqual(json["meal_date"] as? String, "2026-04-17")
        XCTAssertEqual(json["meal_type"] as? String, "dinner")
        XCTAssertNil(json["notes"])
    }

    func testUpdateMealPlanRequestEncodingIncludesEmptyNotes() throws {
        let date = try XCTUnwrap(SharedDateFormatters.localDateOnly.date(from: "2026-04-18"))

        let builder = RamekinAPI.shared.buildMealPlanRequest {
            MealPlansAPI.updateMealPlanWithRequestBuilder(
                id: UUID(uuidString: "87654321-4321-4321-4321-CBA987654321")!,
                updateMealPlanRequest: UpdateMealPlanRequest(
                    mealDate: date,
                    mealType: .lunch,
                    notes: ""
                )
            )
        }
        let json = try requestBodyJSON(from: builder)

        XCTAssertEqual(json["meal_date"] as? String, "2026-04-18")
        XCTAssertEqual(json["meal_type"] as? String, "lunch")
        XCTAssertEqual(json["notes"] as? String, "")
    }

    // MARK: - Response Decoding Tests

    func testLoginResponseDecoding() throws {
        let json = """
        {"token": "abc123xyz"}
        """
        let data = json.data(using: .utf8)!
        let response = try JSONDecoder().decode(LoginResponse.self, from: data)

        XCTAssertEqual(response.token, "abc123xyz")
    }

    func testCreateScrapeResponseDecoding() throws {
        let json = """
        {"id": "12345678-1234-1234-1234-123456789ABC", "status": "pending"}
        """
        let data = json.data(using: .utf8)!
        let response = try JSONDecoder().decode(CreateScrapeResponse.self, from: data)

        XCTAssertEqual(response.id.uuidString, "12345678-1234-1234-1234-123456789ABC")
        XCTAssertEqual(response.status, "pending")
    }

    func testErrorResponseDecoding() throws {
        let json1 = """
        {"code": "internal", "error": "Something went wrong"}
        """
        let data1 = json1.data(using: .utf8)!
        let response1 = try JSONDecoder().decode(ModelErrorResponse.self, from: data1)
        XCTAssertEqual(response1.error, "Something went wrong")
        XCTAssertEqual(response1.code, ._internal)
    }

    func testErrorResponseDecodesStructuredCode() throws {
        let json = """
        {"code": "not_found", "error": "Recipe not found"}
        """
        let data = json.data(using: .utf8)!
        let response = try JSONDecoder().decode(ModelErrorResponse.self, from: data)
        XCTAssertEqual(response.error, "Recipe not found")
        XCTAssertEqual(response.code, .notFound)
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

private func requestBodyJSON<T>(from builder: RequestBuilder<T>) throws -> [String: Any] {
    let data = try XCTUnwrap(builder.parameters?.values.compactMap { $0 as? Data }.first)
    let json = try JSONSerialization.jsonObject(with: data)
    return try XCTUnwrap(json as? [String: Any])
}
