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

    /// Custom URLSession that accepts self-signed certificates
    private lazy var urlSession: URLSession = {
        let config = URLSessionConfiguration.default
        return URLSession(configuration: config, delegate: InsecureSessionDelegate(), delegateQueue: nil)
    }()

    /// Shared URLSession for authenticated image requests.
    lazy var imageSession: URLSession = urlSession

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

        let (data, response) = try await urlSession.data(for: request)

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

        guard let baseURL = serverURL else {
            logger.log("ERROR: No server URL configured")
            throw APIError.noServerURL
        }
        logger.log("Using server URL: \(baseURL)")

        guard let token = authToken else {
            logger.log("ERROR: No auth token")
            throw APIError.noAuthToken
        }
        logger.log("Auth token present (length: \(token.count))")

        guard let url = URL(string: "\(baseURL)/api/scrape") else {
            logger.log("ERROR: Invalid URL: \(baseURL)/api/scrape")
            throw APIError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        applyAccessHeaders(to: &request)

        let body = ScrapeRequest(url: urlString)
        request.httpBody = try JSONEncoder().encode(body)

        logger.log("REQUEST: POST \(url.absoluteString)")
        logger.log("REQUEST BODY: \(String(data: request.httpBody ?? Data(), encoding: .utf8) ?? "nil")")

        do {
            let (data, response) = try await urlSession.data(for: request)

            guard let httpResponse = response as? HTTPURLResponse else {
                logger.log("ERROR: Invalid response (not HTTPURLResponse)")
                throw APIError.invalidResponse
            }

            let responseBody = String(data: data, encoding: .utf8) ?? "nil"
            logger.log("RESPONSE: HTTP \(httpResponse.statusCode)")
            logger.log("RESPONSE BODY: \(responseBody)")

            if httpResponse.statusCode == 200 || httpResponse.statusCode == 201 {
                let decoded = try JSONDecoder().decode(ScrapeResponse.self, from: data)
                logger.log("SUCCESS: Scrape job ID: \(decoded.id)")
                return decoded
            } else {
                let errorMessage: String?
                if let errorResponse = try? JSONDecoder().decode(ErrorResponse.self, from: data) {
                    errorMessage = errorResponse.errorMessage
                } else {
                    errorMessage = responseBody
                }
                logger.log("ERROR: HTTP \(httpResponse.statusCode) - \(errorMessage ?? "unknown")")
                throw APIError.httpError(httpResponse.statusCode, errorMessage)
            }
        } catch let error as APIError {
            throw error
        } catch {
            logger.log("NETWORK ERROR: \(error.localizedDescription)")
            throw APIError.networkError(error)
        }
    }

    /// Check the status of a scrape job
    func getScrapeStatus(id: String) async throws -> ScrapeJobStatus {
        guard let baseURL = serverURL else {
            throw APIError.noServerURL
        }
        guard let token = authToken else {
            throw APIError.noAuthToken
        }
        guard let url = URL(string: "\(baseURL)/api/scrape/\(id)") else {
            throw APIError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        applyAccessHeaders(to: &request)

        let (data, response) = try await urlSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        if httpResponse.statusCode == 200 {
            return try JSONDecoder().decode(ScrapeJobStatus.self, from: data)
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

    // MARK: - Connection Test

    /// Test the connection to the server
    func testConnection() async throws -> Bool {
        guard let baseURL = serverURL else {
            throw APIError.noServerURL
        }
        guard let url = URL(string: "\(baseURL)/api/test/unauthed-ping") else {
            throw APIError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        applyAccessHeaders(to: &request)

        let (_, response) = try await urlSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        return httpResponse.statusCode == 200
    }
}

// MARK: - Meal Plans

extension RamekinAPI {
    func listMealPlans(startDate: Date, endDate: Date) async throws -> MealPlanListResponse {
        guard let baseURL = serverURL else { throw APIError.noServerURL }
        guard let token = authToken else { throw APIError.noAuthToken }

        let start = SharedDateFormatters.localDateOnly.string(from: startDate)
        let end = SharedDateFormatters.localDateOnly.string(from: endDate)

        guard let url = URL(string: "\(baseURL)/api/meal-plans?start_date=\(start)&end_date=\(end)") else {
            throw APIError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        applyAccessHeaders(to: &request)

        let (data, response) = try await urlSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        if httpResponse.statusCode == 200 {
            return try CodableHelper.jsonDecoder.decode(MealPlanListResponse.self, from: data)
        } else {
            throw parseError(from: data, statusCode: httpResponse.statusCode)
        }
    }

    func createMealPlan(
        recipeId: UUID, mealDate: Date, mealType: String, notes: String? = nil
    ) async throws -> CreateMealPlanResponse {
        guard let baseURL = serverURL else { throw APIError.noServerURL }
        guard let token = authToken else { throw APIError.noAuthToken }

        guard let url = URL(string: "\(baseURL)/api/meal-plans") else {
            throw APIError.invalidURL
        }

        let normalizedNotes: String?
        if let notes, !notes.isEmpty {
            normalizedNotes = notes
        } else {
            normalizedNotes = nil
        }

        let body = CreateMealPlanRequestBody(
            recipeId: recipeId,
            mealDate: SharedDateFormatters.localDateOnly.string(from: mealDate),
            mealType: mealType,
            notes: normalizedNotes
        )

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        applyAccessHeaders(to: &request)
        request.httpBody = try JSONEncoder().encode(body)

        let (data, response) = try await urlSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        if httpResponse.statusCode == 200 || httpResponse.statusCode == 201 {
            return try CodableHelper.jsonDecoder.decode(CreateMealPlanResponse.self, from: data)
        } else {
            throw parseError(from: data, statusCode: httpResponse.statusCode)
        }
    }

    func deleteMealPlan(id: UUID) async throws {
        guard let baseURL = serverURL else { throw APIError.noServerURL }
        guard let token = authToken else { throw APIError.noAuthToken }

        guard let url = URL(string: "\(baseURL)/api/meal-plans/\(id.uuidString)") else {
            throw APIError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        applyAccessHeaders(to: &request)

        let (data, response) = try await urlSession.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        if httpResponse.statusCode != 204 && httpResponse.statusCode != 200 {
            throw parseError(from: data, statusCode: httpResponse.statusCode)
        }
    }

    private func parseError(from data: Data, statusCode: Int) -> APIError {
        if let errorResponse = try? JSONDecoder().decode(ErrorResponse.self, from: data) {
            return .httpError(statusCode, errorResponse.errorMessage)
        }
        return .httpError(statusCode, String(data: data, encoding: .utf8))
    }
}
