import Foundation

struct RecipeFormData {
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
    var ingredients: [EditableIngredient] = [.empty()]
    var photoIds: [UUID] = []
    var expectedVersionId: UUID?

    init() {}

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
        ingredients: [EditableIngredient],
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
        self.ingredients = ingredients
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
        ingredients = recipe.ingredients.isEmpty
            ? [.empty()]
            : recipe.ingredients.map { EditableIngredient.from($0) }
    }

    private var validIngredients: [Ingredient] {
        ingredients
            .filter { !$0.item.trimmingCharacters(in: .whitespaces).isEmpty }
            .map { $0.toIngredient() }
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

    func makeUpdateRequest() -> UpdateRecipeRequest {
        guard let expectedVersionId else {
            preconditionFailure("Cannot update a recipe before loading its version")
        }
        UpdateRecipeRequest(
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
