import Foundation

struct CreateMealPlanRequestBody: Encodable {
    let recipeId: UUID
    let mealDate: String
    let mealType: String
    let notes: String?

    enum CodingKeys: String, CodingKey {
        case recipeId = "recipe_id"
        case mealDate = "meal_date"
        case mealType = "meal_type"
        case notes
    }
}

/// API client for interacting with the Ramekin server
class RamekinAPI {
    static let shared = RamekinAPI()

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
        case httpError(Int, String?)
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
            case .httpError(let code, let message):
                return message ?? "HTTP error \(code)"
            case .networkError(let error):
                return "Network error: \(error.localizedDescription)"
            case .decodingError(let error):
                return "Failed to parse response: \(error.localizedDescription)"
            }
        }
    }

    // MARK: - API Types

    struct LoginRequest: Encodable {
        let username: String
        let password: String
    }

    struct LoginResponse: Decodable {
        let token: String
    }

    struct ScrapeRequest: Encodable {
        let url: String
    }

    struct ScrapeResponse: Decodable {
        let id: String
    }

    struct ScrapeJobStatus: Decodable {
        let id: String
        let status: String
        let recipe_id: String?
        let error_message: String?
    }

    struct ErrorResponse: Decodable {
        let error: String?
        let message: String?

        var errorMessage: String {
            error ?? message ?? "Unknown error"
        }
    }

    // MARK: - Request Helper

    /// Execute a request against the configured server, applying the common
    /// guard/URL/Bearer/Access-header boilerplate and translating transport-level
    /// failures into `APIError`. Returns the raw response body on success;
    /// callers decode as needed.
    @discardableResult
    fileprivate func performRequest(
        method: String,
        path: String,
        body: Data? = nil,
        requiresAuth: Bool = true,
        acceptedStatusCodes: Set<Int> = [200, 201, 204]
    ) async throws -> Data {
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
        if let token {
            urlRequest.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        if body != nil {
            urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        applyAccessHeaders(to: &urlRequest)
        urlRequest.httpBody = body

        logger.log("REQUEST: \(method) \(url.absoluteString)")
        if let body, let bodyString = String(data: body, encoding: .utf8) {
            logger.log("REQUEST BODY: \(bodyString)")
        }

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
        return data
    }

    fileprivate func parseError(from data: Data, statusCode: Int) -> APIError {
        if let errorResponse = try? JSONDecoder().decode(ErrorResponse.self, from: data) {
            return .httpError(statusCode, errorResponse.errorMessage)
        }
        return .httpError(statusCode, String(data: data, encoding: .utf8))
    }

    // MARK: - Authentication

    /// Login to the Ramekin server
    func login(
        serverURL: String,
        username: String,
        password: String,
        accessClientId: String? = nil,
        accessClientSecret: String? = nil
    ) async throws -> String {
        // Normalize URL
        var normalizedURL = serverURL.trimmingCharacters(in: .whitespacesAndNewlines)
        if !normalizedURL.hasPrefix("http://") && !normalizedURL.hasPrefix("https://") {
            normalizedURL = "https://\(normalizedURL)"
        }
        if normalizedURL.hasSuffix("/") {
            normalizedURL = String(normalizedURL.dropLast())
        }

        // Persist (or clear) access credentials before the login call so the
        // request itself carries the current CF-Access headers through the
        // Access policy. Empty or nil clears so stale values aren't re-sent.
        if let id = accessClientId, !id.isEmpty {
            _ = KeychainHelper.shared.saveAccessClientId(id)
        } else {
            KeychainHelper.shared.deleteAccessClientId()
        }
        if let secret = accessClientSecret, !secret.isEmpty {
            _ = KeychainHelper.shared.saveAccessClientSecret(secret)
        } else {
            KeychainHelper.shared.deleteAccessClientSecret()
        }

        guard let url = URL(string: "\(normalizedURL)/api/auth/login") else {
            throw APIError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        applyAccessHeaders(to: &request)

        let body = LoginRequest(username: username, password: password)
        request.httpBody = try JSONEncoder().encode(body)

        let (data, response) = try await insecureSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        if httpResponse.statusCode == 200 {
            let loginResponse = try JSONDecoder().decode(LoginResponse.self, from: data)

            // Save credentials
            self.serverURL = normalizedURL
            _ = KeychainHelper.shared.saveToken(loginResponse.token)
            _ = KeychainHelper.shared.saveUsername(username)

            // Update generated client with new credentials
            updateGeneratedClientConfig()

            return loginResponse.token
        } else {
            let errorMessage: String?
            if let errorResponse = try? JSONDecoder().decode(ErrorResponse.self, from: data) {
                errorMessage = errorResponse.errorMessage
            } else {
                errorMessage = String(data: data, encoding: .utf8)
            }
            throw APIError.httpError(httpResponse.statusCode, errorMessage)
        }
    }

    /// Logout and clear credentials
    func logout() {
        KeychainHelper.shared.clearAll()
        updateGeneratedClientConfig()
    }

    // MARK: - Scraping

    /// Submit a URL for scraping (async job)
    func scrapeURL(_ urlString: String) async throws -> ScrapeResponse {
        logger.log("scrapeURL called with: \(urlString)")
        let body = try JSONEncoder().encode(ScrapeRequest(url: urlString))
        let data = try await performRequest(
            method: "POST",
            path: "/api/scrape",
            body: body,
            acceptedStatusCodes: [200, 201]
        )
        let decoded = try JSONDecoder().decode(ScrapeResponse.self, from: data)
        logger.log("SUCCESS: Scrape job ID: \(decoded.id)")
        return decoded
    }

    /// Check the status of a scrape job
    func getScrapeStatus(id: String) async throws -> ScrapeJobStatus {
        let data = try await performRequest(
            method: "GET",
            path: "/api/scrape/\(id)",
            acceptedStatusCodes: [200]
        )
        return try JSONDecoder().decode(ScrapeJobStatus.self, from: data)
    }

    // MARK: - Connection Test

    /// Test the connection to the server. Returns true on 200, false on any
    /// other HTTP status; rethrows network/URL-level errors.
    func testConnection() async throws -> Bool {
        do {
            _ = try await performRequest(
                method: "GET",
                path: "/api/test/unauthed-ping",
                requiresAuth: false,
                acceptedStatusCodes: [200]
            )
            return true
        } catch APIError.httpError {
            return false
        }
    }
}

// MARK: - Meal Plans

extension RamekinAPI {
    private static let dateOnlyFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = .current
        return formatter
    }()

    func listMealPlans(startDate: Date, endDate: Date) async throws -> MealPlanListResponse {
        let start = Self.dateOnlyFormatter.string(from: startDate)
        let end = Self.dateOnlyFormatter.string(from: endDate)
        let data = try await performRequest(
            method: "GET",
            path: "/api/meal-plans?start_date=\(start)&end_date=\(end)",
            acceptedStatusCodes: [200]
        )
        return try CodableHelper.jsonDecoder.decode(MealPlanListResponse.self, from: data)
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

        let body = try JSONEncoder().encode(CreateMealPlanRequestBody(
            recipeId: recipeId,
            mealDate: Self.dateOnlyFormatter.string(from: mealDate),
            mealType: mealType,
            notes: normalizedNotes
        ))
        let data = try await performRequest(
            method: "POST",
            path: "/api/meal-plans",
            body: body,
            acceptedStatusCodes: [200, 201]
        )
        return try CodableHelper.jsonDecoder.decode(CreateMealPlanResponse.self, from: data)
    }

    func deleteMealPlan(id: UUID) async throws {
        try await performRequest(
            method: "DELETE",
            path: "/api/meal-plans/\(id.uuidString)",
            acceptedStatusCodes: [200, 204]
        )
    }
}
