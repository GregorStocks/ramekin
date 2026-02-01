import Foundation

/// API client for interacting with the Ramekin server
class RamekinAPI {
    static let shared = RamekinAPI()

    private let logger = DebugLogger.shared

    private init() {
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
    func login(serverURL: String, username: String, password: String) async throws -> String {
        // Normalize URL
        var normalizedURL = serverURL.trimmingCharacters(in: .whitespacesAndNewlines)
        if !normalizedURL.hasPrefix("http://") && !normalizedURL.hasPrefix("https://") {
            normalizedURL = "https://\(normalizedURL)"
        }
        if normalizedURL.hasSuffix("/") {
            normalizedURL = String(normalizedURL.dropLast())
        }

        guard let url = URL(string: "\(normalizedURL)/api/auth/login") else {
            throw APIError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let body = LoginRequest(username: username, password: password)
        request.httpBody = try JSONEncoder().encode(body)

        let (data, response) = try await URLSession.shared.data(for: request)

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

        let body = ScrapeRequest(url: urlString)
        request.httpBody = try JSONEncoder().encode(body)

        logger.log("REQUEST: POST \(url.absoluteString)")
        logger.log("REQUEST BODY: \(String(data: request.httpBody ?? Data(), encoding: .utf8) ?? "nil")")

        do {
            let (data, response) = try await URLSession.shared.data(for: request)

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

        let (data, response) = try await URLSession.shared.data(for: request)

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

        let (_, response) = try await URLSession.shared.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        return httpResponse.statusCode == 200
    }
}
