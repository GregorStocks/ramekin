import Foundation

enum RecipeFormError: LocalizedError {
    /// Saving an edit needs the version it was based on.
    case versionNotLoaded

    var errorDescription: String? {
        switch self {
        case .versionNotLoaded:
            return "Reload the recipe and try again."
        }
    }
}

struct RecipeFormData: Equatable {
    var title: String = ""
    var recipeDescription: String = ""
    var instructions: String = ""
    var servings: String = ""
    var prepTime: String = ""
    var cookTime: String = ""
    var totalTime: String = ""
    var difficulty: String = ""
    var rating: Int?
    var sourceUrl: String = ""
    var sourceName: String = ""
    var tags: [String] = []
    var notes: String = ""
    var nutritionalInfo: String = ""
    var ingredientRows: [IngredientEditorRow] = [.ingredient(.empty())]
    var photoIds: [UUID] = []
    var expectedVersionId: UUID?

    init() {}

    init(content: RecipeContent) {
        title = content.title
        recipeDescription = content.description ?? ""
        instructions = content.instructions
        servings = content.servings ?? ""
        prepTime = content.prepTime ?? ""
        cookTime = content.cookTime ?? ""
        totalTime = content.totalTime ?? ""
        difficulty = content.difficulty ?? ""
        rating = content.rating
        sourceUrl = content.sourceUrl ?? ""
        sourceName = content.sourceName ?? ""
        tags = content.tags ?? []
        notes = content.notes ?? ""
        nutritionalInfo = content.nutritionalInfo ?? ""
        ingredientRows = ingredientEditorRows(from: content.ingredients)
    }

    init(
        title: String,
        recipeDescription: String,
        instructions: String,
        servings: String,
        prepTime: String,
        cookTime: String,
        totalTime: String,
        difficulty: String,
        rating: Int?,
        sourceUrl: String,
        sourceName: String,
        tags: [String],
        notes: String,
        nutritionalInfo: String,
        ingredientRows: [IngredientEditorRow],
        photoIds: [UUID],
        expectedVersionId: UUID? = nil
    ) {
        self.title = title
        self.recipeDescription = recipeDescription
        self.instructions = instructions
        self.servings = servings
        self.prepTime = prepTime
        self.cookTime = cookTime
        self.totalTime = totalTime
        self.difficulty = difficulty
        self.rating = rating
        self.sourceUrl = sourceUrl
        self.sourceName = sourceName
        self.tags = tags
        self.notes = notes
        self.nutritionalInfo = nutritionalInfo
        self.ingredientRows = ingredientRows
        self.photoIds = photoIds
        self.expectedVersionId = expectedVersionId
    }

    init(recipe: RecipeResponse) {
        title = recipe.title
        recipeDescription = recipe.description ?? ""
        instructions = recipe.instructions
        servings = recipe.servings ?? ""
        prepTime = recipe.prepTime ?? ""
        cookTime = recipe.cookTime ?? ""
        totalTime = recipe.totalTime ?? ""
        difficulty = recipe.difficulty ?? ""
        rating = recipe.rating
        sourceUrl = recipe.sourceUrl ?? ""
        sourceName = recipe.sourceName ?? ""
        tags = recipe.tags
        notes = recipe.notes ?? ""
        nutritionalInfo = recipe.nutritionalInfo ?? ""
        photoIds = recipe.photoIds
        expectedVersionId = recipe.versionId
        ingredientRows = recipe.ingredients.isEmpty
            ? [.ingredient(.empty())]
            : ingredientEditorRows(from: recipe.ingredients)
    }

    private var validIngredients: [Ingredient] {
        ingredients(from: ingredientRows)
            .filter { !$0.item.trimmingCharacters(in: .whitespaces).isEmpty }
    }

    func makeCreateRequest() -> CreateRecipeRequest {
        CreateRecipeRequest(
            cookTime: cookTime.isEmpty ? nil : cookTime,
            description: recipeDescription.isEmpty ? nil : recipeDescription,
            difficulty: difficulty.isEmpty ? nil : difficulty,
            ingredients: validIngredients,
            instructions: instructions,
            notes: notes.isEmpty ? nil : notes,
            nutritionalInfo: nutritionalInfo.isEmpty ? nil : nutritionalInfo,
            prepTime: prepTime.isEmpty ? nil : prepTime,
            rating: rating,
            servings: servings.isEmpty ? nil : servings,
            sourceName: sourceName.isEmpty ? nil : sourceName,
            sourceUrl: sourceUrl.isEmpty ? nil : sourceUrl,
            tags: tags.isEmpty ? nil : tags,
            title: title,
            totalTime: totalTime.isEmpty ? nil : totalTime,
            photoIds: photoIds.isEmpty ? nil : photoIds
        )
    }

    func makeUpdateRequest() throws -> UpdateRecipeRequest {
        guard let expectedVersionId else {
            throw RecipeFormError.versionNotLoaded
        }
        return UpdateRecipeRequest(
            cookTime: cookTime.isEmpty ? nil : cookTime,
            description: recipeDescription.isEmpty ? nil : recipeDescription,
            difficulty: difficulty.isEmpty ? nil : difficulty,
            expectedVersionId: expectedVersionId,
            ingredients: validIngredients,
            instructions: instructions,
            notes: notes.isEmpty ? nil : notes,
            nutritionalInfo: nutritionalInfo.isEmpty ? nil : nutritionalInfo,
            photoIds: photoIds,
            prepTime: prepTime.isEmpty ? nil : prepTime,
            rating: rating,
            servings: servings.isEmpty ? nil : servings,
            sourceName: sourceName.isEmpty ? nil : sourceName,
            sourceUrl: sourceUrl.isEmpty ? nil : sourceUrl,
            tags: tags,
            title: title,
            totalTime: totalTime.isEmpty ? nil : totalTime
        )
    }
}
