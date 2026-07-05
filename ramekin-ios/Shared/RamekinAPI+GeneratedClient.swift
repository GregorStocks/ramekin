import Foundation

private let generatedDateFormatterLock = NSRecursiveLock()

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
    }

    func buildMealPlanRequest<T>(_ makeBuilder: () -> RequestBuilder<T>) -> RequestBuilder<T> {
        generatedDateFormatterLock.lock()
        defer { generatedDateFormatterLock.unlock() }

        let previousDateFormatter = CodableHelper.dateFormatter
        let previousEncoder = CodableHelper.jsonEncoder
        CodableHelper.dateFormatter = SharedDateFormatters.localDateOnly
        let dateOnlyEncoder = JSONEncoder()
        dateOnlyEncoder.dateEncodingStrategy = .formatted(SharedDateFormatters.localDateOnly)
        dateOnlyEncoder.outputFormatting = .prettyPrinted
        CodableHelper.jsonEncoder = dateOnlyEncoder
        defer {
            CodableHelper.dateFormatter = previousDateFormatter
            CodableHelper.jsonEncoder = previousEncoder
        }

        return makeBuilder()
    }

    func generatedMealType(from rawValue: String) -> MealType {
        guard let mealType = MealType(rawValue: rawValue) else {
            fatalError("Unsupported meal type: \(rawValue)")
        }
        return mealType
    }
}
