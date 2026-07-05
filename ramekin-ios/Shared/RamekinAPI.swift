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

        var errorDescription: String? {
            switch self {
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

    /// Login to the Ramekin server
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

        // Persist (or clear) access credentials before the login call so the
        // request itself carries the current CF-Access headers through the
        // Access policy. Empty or nil clears so stale values aren't re-sent.
        if let id = normalizedAccessClientId {
            _ = credentialStore.saveAccessClientId(id)
        } else {
            credentialStore.deleteAccessClientId()
        }
        if let secret = normalizedAccessClientSecret {
            _ = credentialStore.saveAccessClientSecret(secret)
        } else {
            credentialStore.deleteAccessClientSecret()
        }

        RamekinClientAPI.basePath = normalizedURL
        RamekinClientAPI.customHeaders.removeValue(forKey: "Authorization")
        if let id = normalizedAccessClientId {
            RamekinClientAPI.customHeaders["CF-Access-Client-Id"] = id
        } else {
            RamekinClientAPI.customHeaders.removeValue(forKey: "CF-Access-Client-Id")
        }
        if let secret = normalizedAccessClientSecret {
            RamekinClientAPI.customHeaders["CF-Access-Client-Secret"] = secret
        } else {
            RamekinClientAPI.customHeaders.removeValue(forKey: "CF-Access-Client-Secret")
        }

        let builder = AuthAPI.loginWithRequestBuilder(
            loginRequest: LoginRequest(password: password, username: username)
        )

        do {
            let loginResponse = try await requestExecutor(builder)
            // Save credentials
            _ = credentialStore.saveServerURL(normalizedURL)
            _ = credentialStore.saveToken(loginResponse.token)
            _ = credentialStore.saveUsername(username)

            // Update generated client with new credentials
            if updateClientConfiguration {
                updateGeneratedClientConfig()
            }

            return loginResponse.token
        } catch {
            throw translateGeneratedError(error)
        }
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
    func listMealPlans(startDate: Date, endDate: Date) async throws -> MealPlanListResponse {
        try await executeGenerated {
            try await buildMealPlanRequest {
                MealPlansAPI.listMealPlansWithRequestBuilder(
                    startDate: startDate,
                    endDate: endDate
                )
            }.execute().body
        }
    }

    func createMealPlan(
        recipeId: UUID, mealDate: Date, mealType: String, notes: String? = nil
    ) async throws -> CreateMealPlanResponse {
        let normalizedNotes: String?
        if let notes, !notes.isEmpty {
            normalizedNotes = notes
        } else {
            normalizedNotes = nil
        }

        let request = CreateMealPlanRequest(
            mealDate: mealDate,
            mealType: generatedMealType(from: mealType),
            notes: normalizedNotes,
            recipeId: recipeId
        )
        return try await executeGenerated {
            try await buildMealPlanRequest {
                MealPlansAPI.createMealPlanWithRequestBuilder(createMealPlanRequest: request)
            }.execute().body
        }
    }

    func deleteMealPlan(id: UUID) async throws {
        try await executeGenerated {
            try await MealPlansAPI.deleteMealPlan(id: id)
        }
    }

    func updateMealPlan(
        id: UUID, mealDate: Date, mealType: String, notes: String
    ) async throws {
        let request = UpdateMealPlanRequest(
            mealDate: mealDate,
            mealType: generatedMealType(from: mealType),
            notes: notes
        )
        try await executeGenerated {
            try await buildMealPlanRequest {
                MealPlansAPI.updateMealPlanWithRequestBuilder(
                    id: id,
                    updateMealPlanRequest: request
                )
            }.execute().body
        }
    }
}
