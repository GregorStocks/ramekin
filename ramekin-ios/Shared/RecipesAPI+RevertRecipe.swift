import Foundation

extension RecipesAPI {
    @available(macOS 10.15, iOS 13.0, tvOS 13.0, watchOS 6.0, *)
    static func revertRecipe(
        id: UUID,
        recipe: RecipeResponse,
        expectedVersionId: UUID
    ) async throws {
        let request = RevertRecipeRequest(recipe: recipe, expectedVersionId: expectedVersionId)
        _ = try await revertRecipeWithRequestBuilder(id: id, request: request).execute().body
    }

    static func revertRecipeWithRequestBuilder(
        id: UUID,
        request: RevertRecipeRequest
    ) -> RequestBuilder<Void> {
        var localVariablePath = "/api/recipes/{id}"
        let idPreEscape = "\(APIHelper.mapValueToPathItem(id))"
        let idPostEscape = idPreEscape.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? ""
        localVariablePath = localVariablePath.replacingOccurrences(
            of: "{id}",
            with: idPostEscape,
            options: .literal,
            range: nil
        )

        let localVariableURLString = RamekinClientAPI.basePath + localVariablePath
        let localVariableParameters = JSONEncodingHelper.encodingParameters(forEncodableObject: request)
        let localVariableUrlComponents = URLComponents(string: localVariableURLString)
        let localVariableNillableHeaders: [String: Any?] = [
            "Content-Type": "application/json"
        ]
        let localVariableHeaderParameters = APIHelper.rejectNilHeaders(localVariableNillableHeaders)
        let localVariableRequestBuilder: RequestBuilder<Void>.Type = RamekinClientAPI.requestBuilderFactory
            .getNonDecodableBuilder()

        return localVariableRequestBuilder.init(
            method: "PUT",
            URLString: (localVariableUrlComponents?.string ?? localVariableURLString),
            parameters: localVariableParameters,
            headers: localVariableHeaderParameters,
            requiresAuthentication: true
        )
    }
}

struct RevertRecipeRequest: Encodable {
    let cookTime: String?
    let description: String?
    let difficulty: String?
    let expectedVersionId: UUID
    let ingredients: [Ingredient]
    let instructions: String
    let notes: String?
    let nutritionalInfo: String?
    let photoIds: [UUID]
    let prepTime: String?
    let rating: Int?
    let servings: String?
    let sourceName: String?
    let sourceUrl: String?
    let tags: [String]
    let title: String
    let totalTime: String?

    init(recipe: RecipeResponse, expectedVersionId: UUID) {
        cookTime = recipe.cookTime
        description = recipe.description
        difficulty = recipe.difficulty
        self.expectedVersionId = expectedVersionId
        ingredients = recipe.ingredients
        instructions = recipe.instructions
        notes = recipe.notes
        nutritionalInfo = recipe.nutritionalInfo
        photoIds = recipe.photoIds
        prepTime = recipe.prepTime
        rating = recipe.rating
        servings = recipe.servings
        sourceName = recipe.sourceName
        sourceUrl = recipe.sourceUrl
        tags = recipe.tags
        title = recipe.title
        totalTime = recipe.totalTime
    }

    enum CodingKeys: String, CodingKey {
        case cookTime = "cook_time"
        case description
        case difficulty
        case expectedVersionId = "expected_version_id"
        case ingredients
        case instructions
        case notes
        case nutritionalInfo = "nutritional_info"
        case photoIds = "photo_ids"
        case prepTime = "prep_time"
        case rating
        case servings
        case sourceName = "source_name"
        case sourceUrl = "source_url"
        case tags
        case title
        case totalTime = "total_time"
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)

        try container.encode(cookTime, forKey: .cookTime)
        try encodeNullable(description, forKey: .description, into: &container)
        try encodeNullable(difficulty, forKey: .difficulty, into: &container)
        try container.encode(expectedVersionId, forKey: .expectedVersionId)
        try container.encode(ingredients, forKey: .ingredients)
        try container.encode(instructions, forKey: .instructions)
        try encodeNullable(notes, forKey: .notes, into: &container)
        try encodeNullable(nutritionalInfo, forKey: .nutritionalInfo, into: &container)
        try container.encode(photoIds, forKey: .photoIds)
        try encodeNullable(prepTime, forKey: .prepTime, into: &container)
        try encodeNullable(rating, forKey: .rating, into: &container)
        try encodeNullable(servings, forKey: .servings, into: &container)
        try encodeNullable(sourceName, forKey: .sourceName, into: &container)
        try encodeNullable(sourceUrl, forKey: .sourceUrl, into: &container)
        try container.encode(tags, forKey: .tags)
        try container.encode(title, forKey: .title)
        try encodeNullable(totalTime, forKey: .totalTime, into: &container)
    }

    private func encodeNullable<T: Encodable>(
        _ value: T?,
        forKey key: CodingKeys,
        into container: inout KeyedEncodingContainer<CodingKeys>
    ) throws {
        if let value {
            try container.encode(value, forKey: key)
        } else {
            try container.encodeNil(forKey: key)
        }
    }
}
