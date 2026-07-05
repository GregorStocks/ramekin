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

    func generatedMealType(from rawValue: String) -> MealType {
        guard let mealType = MealType(rawValue: rawValue) else {
            fatalError("Unsupported meal type: \(rawValue)")
        }
        return mealType
    }

    func listMealPlansRequestBuilder(startDate: Date, endDate: Date) -> RequestBuilder<MealPlanListResponse> {
        let urlString = RamekinClientAPI.basePath + "/api/meal-plans"
        var urlComponents = URLComponents(string: urlString)
        urlComponents?.queryItems = APIHelper.mapValuesToQueryItems([
            "start_date": (
                wrappedValue: SharedDateFormatters.localDateOnly.string(from: startDate),
                isExplode: true
            ),
            "end_date": (
                wrappedValue: SharedDateFormatters.localDateOnly.string(from: endDate),
                isExplode: true
            )
        ])

        let builderType: RequestBuilder<MealPlanListResponse>.Type = RamekinClientAPI
            .requestBuilderFactory
            .getBuilder()
        return builderType.init(
            method: "GET",
            URLString: urlComponents?.string ?? urlString,
            parameters: nil,
            headers: [:],
            requiresAuthentication: true
        )
    }

    func createMealPlanRequestBuilder(
        _ request: CreateMealPlanRequest
    ) -> RequestBuilder<CreateMealPlanResponse> {
        var body: [String: Any] = [
            CreateMealPlanRequest.CodingKeys.mealDate.rawValue: SharedDateFormatters.localDateOnly
                .string(from: request.mealDate),
            CreateMealPlanRequest.CodingKeys.mealType.rawValue: request.mealType.rawValue,
            CreateMealPlanRequest.CodingKeys.recipeId.rawValue: request.recipeId.uuidString
        ]
        if let notes = request.notes {
            body[CreateMealPlanRequest.CodingKeys.notes.rawValue] = notes
        }

        let builderType: RequestBuilder<CreateMealPlanResponse>.Type = RamekinClientAPI
            .requestBuilderFactory
            .getBuilder()
        return builderType.init(
            method: "POST",
            URLString: RamekinClientAPI.basePath + "/api/meal-plans",
            parameters: jsonParameters(from: body),
            headers: ["Content-Type": "application/json"],
            requiresAuthentication: true
        )
    }

    func updateMealPlanRequestBuilder(
        id: UUID,
        request: UpdateMealPlanRequest
    ) -> RequestBuilder<Void> {
        var body: [String: Any] = [:]
        if let mealDate = request.mealDate {
            body[UpdateMealPlanRequest.CodingKeys.mealDate.rawValue] = SharedDateFormatters.localDateOnly
                .string(from: mealDate)
        }
        if let mealType = request.mealType {
            body[UpdateMealPlanRequest.CodingKeys.mealType.rawValue] = mealType.rawValue
        }
        if let notes = request.notes {
            body[UpdateMealPlanRequest.CodingKeys.notes.rawValue] = notes
        }

        let builderType: RequestBuilder<Void>.Type = RamekinClientAPI
            .requestBuilderFactory
            .getNonDecodableBuilder()
        return builderType.init(
            method: "PUT",
            URLString: RamekinClientAPI.basePath + "/api/meal-plans/\(id.uuidString)",
            parameters: jsonParameters(from: body),
            headers: ["Content-Type": "application/json"],
            requiresAuthentication: true
        )
    }

    private func jsonParameters(from body: [String: Any]) -> [String: Any]? {
        do {
            let data = try JSONSerialization.data(withJSONObject: body, options: .prettyPrinted)
            return JSONDataEncoding.encodingParameters(jsonData: data)
        } catch {
            fatalError("Could not encode meal plan request body: \(error)")
        }
    }
}
