import Foundation

extension RamekinAPI {
    func parseError(from data: Data, statusCode: Int) -> APIError {
        if let errorResponse = try? JSONDecoder().decode(ModelErrorResponse.self, from: data) {
            return .httpError(statusCode, errorResponse.code, errorResponse.error)
        }
        return .httpError(statusCode, nil, String(data: data, encoding: .utf8))
    }

    func translateGeneratedError(_ error: Error) -> APIError {
        if let apiError = error as? APIError {
            return apiError
        }
        if let generatedError = error as? ErrorResponse {
            switch generatedError {
            case .error(let statusCode, let data, _, let underlyingError):
                if statusCode == -1 {
                    return .networkError(underlyingError)
                }
                if statusCode == -2 {
                    return .invalidResponse
                }
                if case DecodableRequestBuilderError.jsonDecoding(_) = underlyingError {
                    return .decodingError(underlyingError)
                }
                if let data {
                    return parseError(from: data, statusCode: statusCode)
                }
                return .httpError(statusCode, nil, underlyingError.localizedDescription)
            }
        }
        return .networkError(error)
    }

    func executeGenerated<T>(_ operation: () async throws -> T) async throws -> T {
        updateGeneratedClientConfig()
        do {
            return try await operation()
        } catch {
            throw translateGeneratedError(error)
        }
    }

    func executeGenerated<T>(
        timeoutInterval: TimeInterval?,
        operation: @escaping @Sendable () async throws -> T
    ) async throws -> T {
        guard let timeoutInterval else {
            return try await executeGenerated(operation)
        }
        return try await executeGenerated {
            try await Self.race(operation, againstTimeout: timeoutInterval)
        }
    }

    /// Race `operation` against a wall-clock timeout, throwing
    /// `APIError.networkError(URLError(.timedOut))` if the timeout expires first.
    static func race<T>(
        _ operation: @escaping @Sendable () async throws -> T,
        againstTimeout timeoutInterval: TimeInterval
    ) async throws -> T {
        try await withThrowingTaskGroup(of: T.self) { group in
            group.addTask {
                try await operation()
            }
            group.addTask {
                let nanoseconds = UInt64(timeoutInterval * 1_000_000_000)
                try await Task.sleep(nanoseconds: nanoseconds)
                throw APIError.networkError(URLError(.timedOut))
            }
            guard let result = try await group.next() else {
                throw APIError.invalidResponse
            }
            group.cancelAll()
            return result
        }
    }

    /// The meal-plan sync request, built by hand: the generated encoder
    /// writes a `Date` as a UTC timestamp, but a meal's date is a local day.
    func syncMealPlansRequestBuilder(
        cursor: Int64?,
        ops: [[String: Any]]
    ) throws -> RequestBuilder<SyncMealPlansResponse> {
        var body: [String: Any] = [
            SyncMealPlansRequest.CodingKeys.ops.rawValue: ops
        ]
        if let cursor {
            body[SyncMealPlansRequest.CodingKeys.cursor.rawValue] = cursor
        }
        let data = try JSONSerialization.data(withJSONObject: body)

        let builderType: RequestBuilder<SyncMealPlansResponse>.Type = RamekinClientAPI
            .requestBuilderFactory
            .getBuilder()
        return builderType.init(
            method: "POST",
            URLString: RamekinClientAPI.basePath + "/api/sync/meal-plans",
            parameters: JSONDataEncoding.encodingParameters(jsonData: data),
            headers: ["Content-Type": "application/json"],
            requiresAuthentication: true
        )
    }
}
