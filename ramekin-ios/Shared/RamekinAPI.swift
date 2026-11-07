import Foundation

/// API client for interacting with the Ramekin server
class RamekinAPI {
    static let shared = RamekinAPI()
    typealias LoginExecutor = @Sendable (RequestBuilder<LoginResponse>) async throws -> LoginResponse

    private let logger = DebugLogger.shared

    private init() {
        // Configure generated client to accept self-signed certificates
        RamekinClientAPI.requestBuilderFactory = InsecureBuilderFactory()
        // Configure generated client with any existing credentials
        updateGeneratedClientConfig()
    }

    // MARK: - Configuration

    var serverURL: String? {
        get { KeychainHelper.shared.getServerURL() }
        set {
            if let url = newValue {
                _ = KeychainHelper.shared.saveServerURL(url)
            }
            updateGeneratedClientConfig()
        }
    }

    var authToken: String? {
        KeychainHelper.shared.getToken()
    }

    var accessClientId: String? {
        KeychainHelper.shared.getAccessClientId()
    }

    var accessClientSecret: String? {
        KeychainHelper.shared.getAccessClientSecret()
    }

    var isLoggedIn: Bool {
        authToken != nil && serverURL != nil
    }

    /// Configure the generated OpenAPI client with current credentials
    func updateGeneratedClientConfig() {
        if let baseURL = serverURL {
            RamekinClientAPI.basePath = baseURL
        }
        if let token = authToken {
            RamekinClientAPI.customHeaders["Authorization"] = "Bearer \(token)"
        } else {
            RamekinClientAPI.customHeaders.removeValue(forKey: "Authorization")
        }
        if let id = accessClientId {
            RamekinClientAPI.customHeaders["CF-Access-Client-Id"] = id
        } else {
            RamekinClientAPI.customHeaders.removeValue(forKey: "CF-Access-Client-Id")
        }
        if let secret = accessClientSecret {
            RamekinClientAPI.customHeaders["CF-Access-Client-Secret"] = secret
        } else {
            RamekinClientAPI.customHeaders.removeValue(forKey: "CF-Access-Client-Secret")
        }
    }

    /// Attach Cloudflare Access service-token headers to a request if configured.
    func applyAccessHeaders(to request: inout URLRequest) {
        applyAccessHeaders(
            to: &request,
            accessClientId: accessClientId,
            accessClientSecret: accessClientSecret
        )
    }

    func applyAccessHeaders(
        to request: inout URLRequest,
        accessClientId: String?,
        accessClientSecret: String?
    ) {
        if let id = accessClientId {
            request.setValue(id, forHTTPHeaderField: "CF-Access-Client-Id")
        }
        if let secret = accessClientSecret {
            request.setValue(secret, forHTTPHeaderField: "CF-Access-Client-Secret")
        }
    }

    // MARK: - API Errors

    enum APIError: LocalizedError {
        case noServerURL
        case noAuthToken
        case invalidURL
        case invalidResponse
        /// An HTTP error: status, machine-readable code (if the server returned
        /// one), and human-readable message.
        case httpError(Int, ErrorCode?, String?)
        case networkError(Error)
        case decodingError(Error)
        /// The server accepted the sign-in but the keychain refused to store it.
        case credentialStorageFailed

        var errorDescription: String? {
            switch self {
            case .credentialStorageFailed:
                return "Signed in, but couldn't save the sign-in to the keychain. Please try again."
            case .noServerURL:
                return "No server URL configured"
            case .noAuthToken:
                return "Not logged in"
            case .invalidURL:
                return "Invalid URL"
            case .invalidResponse:
                return "Invalid response from server"
            case .httpError(let status, _, let message):
                return message ?? "HTTP error \(status)"
            case .networkError(let error):
                return "Network error: \(error.localizedDescription)"
            case .decodingError(let error):
                return "Failed to parse response: \(error.localizedDescription)"
            }
        }

        /// The structured error code the server returned, if any. Branch on this
        /// rather than the HTTP status or the message text.
        var code: ErrorCode? {
            if case let .httpError(_, code, _) = self {
                return code
            }
            return nil
        }
    }

    // MARK: - API Types

    // MARK: - Request Helper

    /// Execute a request against the configured server, applying the common
    /// guard/URL/Bearer/Access-header boilerplate and translating transport-level
    /// failures into `APIError`. Returns the raw response body on success;
    /// callers decode as needed.
    @discardableResult
    func performRequest(
        method: String,
        path: String,
        body: Data? = nil,
        requiresAuth: Bool = true,
        acceptedStatusCodes: Set<Int> = [200, 201, 204],
        timeoutInterval: TimeInterval? = nil,
        logBody: Bool = true
    ) async throws -> Data {
        let (data, _) = try await performRequestWithResponse(
            method: method,
            path: path,
            body: body,
            requiresAuth: requiresAuth,
            acceptedStatusCodes: acceptedStatusCodes,
            timeoutInterval: timeoutInterval,
            logBody: logBody
        )
        return data
    }

    /// Like `performRequest`, but also returns the `HTTPURLResponse` so callers
    /// can inspect response headers (e.g. Content-Disposition for downloads).
    func performRequestWithResponse(
        method: String,
        path: String,
        body: Data? = nil,
        requiresAuth: Bool = true,
        acceptedStatusCodes: Set<Int> = [200, 201, 204],
        timeoutInterval: TimeInterval? = nil,
        logBody: Bool = true
    ) async throws -> (Data, HTTPURLResponse) {
        guard let baseURL = serverURL else {
            logger.log("ERROR: No server URL configured")
            throw APIError.noServerURL
        }
        let token = authToken
        if requiresAuth, token == nil {
            logger.log("ERROR: No auth token")
            throw APIError.noAuthToken
        }
        guard let url = URL(string: "\(baseURL)\(path)") else {
            logger.log("ERROR: Invalid URL: \(baseURL)\(path)")
            throw APIError.invalidURL
        }

        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = method
        if let timeoutInterval {
            urlRequest.timeoutInterval = timeoutInterval
        }
        if let token {
            urlRequest.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        if body != nil {
            urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        applyAccessHeaders(to: &urlRequest)
        urlRequest.httpBody = body

        logger.log("REQUEST: \(method) \(url.absoluteString)")
        logRequestBody(body, logBody: logBody)

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await insecureSession.data(for: urlRequest)
        } catch {
            logger.log("NETWORK ERROR: \(error.localizedDescription)")
            throw APIError.networkError(error)
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            logger.log("ERROR: Invalid response (not HTTPURLResponse)")
            throw APIError.invalidResponse
        }

        let responseBody = String(data: data, encoding: .utf8) ?? "nil"
        logger.log("RESPONSE: HTTP \(httpResponse.statusCode)")
        logger.log("RESPONSE BODY: \(responseBody)")

        guard acceptedStatusCodes.contains(httpResponse.statusCode) else {
            AuthExpiry.notifyIfRejected(statusCode: httpResponse.statusCode, sentToken: token)
            throw parseError(from: data, statusCode: httpResponse.statusCode)
        }
        return (data, httpResponse)
    }

    private func logRequestBody(_ body: Data?, logBody: Bool) {
        guard let body else { return }
        if logBody, let bodyString = String(data: body, encoding: .utf8) {
            logger.log("REQUEST BODY: \(bodyString)")
        } else {
            logger.log("REQUEST BODY: <\(body.count) bytes, not logged>")
        }
    }

    // MARK: - Authentication

    /// Timeout for the login POST. Login is a single small round-trip, so a
    /// stall this long means the server is unreachable and the user should see
    /// an error instead of an indefinite spinner. UI tests rely on this bound:
    /// their wait for the login error message must comfortably exceed it
    /// (RecipeFlowTests.testLoginFailure).
    static let loginTimeout: TimeInterval = 15

    /// Login to the Ramekin server. Nothing global changes until the server
    /// accepts the credentials: the request carries the form's server URL and
    /// Access credentials directly, and only a successful login saves them.
    /// If the keychain refuses any write, the token is removed so the app
    /// stays signed out instead of half-switched, and the login throws.
    func login(
        serverURL: String,
        username: String,
        password: String,
        accessClientId: String? = nil,
        accessClientSecret: String? = nil,
        credentialStore: CredentialStore = KeychainHelper.shared,
        updateClientConfiguration: Bool = true,
        requestExecutor: @escaping LoginExecutor = { builder in
            try await builder.execute().body
        }
    ) async throws -> String {
        let normalizedURL = Self.normalizeServerURL(serverURL)
        let normalizedAccessClientId = Self.normalizedOptionalCredential(accessClientId)
        let normalizedAccessClientSecret = Self.normalizedOptionalCredential(accessClientSecret)

        let builder = Self.loginRequestBuilder(
            serverURL: normalizedURL,
            loginRequest: LoginRequest(password: password, username: username),
            accessClientId: normalizedAccessClientId,
            accessClientSecret: normalizedAccessClientSecret
        )

        let loginResponse: LoginResponse
        do {
            loginResponse = try await Self.race(
                { try await requestExecutor(builder) },
                againstTimeout: Self.loginTimeout
            )
        } catch {
            throw translateGeneratedError(error)
        }

        let session = SessionCredentials(
            serverURL: normalizedURL,
            token: loginResponse.token,
            username: username,
            accessClientId: normalizedAccessClientId,
            accessClientSecret: normalizedAccessClientSecret
        )
        let saved = session.save(in: credentialStore)
        if updateClientConfiguration {
            updateGeneratedClientConfig()
        }
        guard saved else {
            logger.log("ERROR: Keychain refused to store the new session")
            throw APIError.credentialStorageFailed
        }
        return loginResponse.token
    }

    static func normalizeServerURL(_ serverURL: String) -> String {
        var normalizedURL = serverURL.trimmingCharacters(in: .whitespacesAndNewlines)
        if !normalizedURL.hasPrefix("http://") && !normalizedURL.hasPrefix("https://") {
            normalizedURL = "https://\(normalizedURL)"
        }
        if normalizedURL.hasSuffix("/") {
            normalizedURL = String(normalizedURL.dropLast())
        }
        return normalizedURL
    }

    private static func normalizedOptionalCredential(_ value: String?) -> String? {
        guard let value else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    /// Adopt a session that was signed in outside the app. UI tests that
    /// aren't about the login form use this to start on the recipe list
    /// without typing credentials.
    func adoptSession(serverURL: String, token: String, username: String) {
        _ = KeychainHelper.shared.saveServerURL(Self.normalizeServerURL(serverURL))
        _ = KeychainHelper.shared.saveToken(token)
        _ = KeychainHelper.shared.saveUsername(username)
        updateGeneratedClientConfig()
    }

    /// Logout and clear credentials
    func logout() {
        KeychainHelper.shared.clearAll()
        updateGeneratedClientConfig()
    }

    /// Timeout for the capture POST. The auth pre-flight (`authCheckTimeout`,
    /// 5s) runs first in the share extension, so pre-flight + upload together
    /// must fit inside the ~30s iOS gives a share extension before terminating
    /// it (5 + 23 = 28). Still ample for multi-MB HTML on a slow mobile uplink.
    static let captureSubmitTimeout: TimeInterval = 23
}

// MARK: - Connection Test

extension RamekinAPI {
    /// Test the connection to the server. Returns true on 200, false on any
    /// other HTTP status; rethrows network/URL-level errors.
    func testConnection() async throws -> Bool {
        do {
            _ = try await executeGenerated { try await TestingAPI.unauthedPing() }
            return true
        } catch APIError.httpError {
            return false
        }
    }
}

// MARK: - Meal Plans

extension RamekinAPI {
    /// `POST /api/sync/meal-plans`. `ops` are the request's op objects, their
    /// dates already the API's `yyyy-MM-dd` days.
    func syncMealPlans(cursor: Int64?, ops: [[String: Any]]) async throws -> SyncMealPlansResponse {
        // Built inside, after the client configuration is refreshed.
        try await executeGenerated {
            try await syncMealPlansRequestBuilder(cursor: cursor, ops: ops).execute().body
        }
    }
}
