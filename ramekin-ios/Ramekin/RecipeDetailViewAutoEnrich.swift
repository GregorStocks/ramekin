import SwiftUI

extension RecipeDetailView {
    var autoEnrichmentProgressLabel: String? {
        if isEnriching { return "Enriching recipe..." }
        if isGeneratingPhoto { return "Generating AI photo..." }
        if isGeneratingDescription { return "Generating description..." }
        if isNormalizingTitle { return "Renaming recipe..." }
        return nil
    }

    func autoEnrichmentProgressBanner(_ label: String) -> some View {
        HStack(spacing: 10) {
            ProgressView()
            Text(label)
                .font(.subheadline)
                .foregroundColor(.primary)
            Spacer()
        }
        .padding(12)
        .background(Color.purple.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    @MainActor
    func enrichWithAI() async {
        guard let recipe else { return }
        isEnriching = true
        autoEnrichError = nil

        let content = RecipeContent(
            cookTime: recipe.cookTime,
            description: recipe.description,
            difficulty: recipe.difficulty,
            ingredients: recipe.ingredients,
            instructions: recipe.instructions,
            notes: recipe.notes,
            nutritionalInfo: recipe.nutritionalInfo,
            prepTime: recipe.prepTime,
            rating: recipe.rating,
            servings: recipe.servings,
            sourceName: recipe.sourceName,
            sourceUrl: recipe.sourceUrl,
            tags: recipe.tags,
            title: recipe.title,
            totalTime: recipe.totalTime
        )

        do {
            let enriched = try await EnrichAPI.enrichRecipe(recipeContent: content)
            enrichResult = enriched
        } catch is CancellationError {
        } catch {
            autoEnrichError = "Failed to enrich recipe"
        }
        isEnriching = false
    }

    @MainActor
    func generatePhoto() async {
        guard recipe != nil else { return }
        isGeneratingPhoto = true
        autoEnrichError = nil

        do {
            _ = try await RecipesAPI.generatePhoto(id: recipeId)
            await loadRecipe()
        } catch is CancellationError {
        } catch {
            autoEnrichError = "Failed to generate AI photo"
        }
        isGeneratingPhoto = false
    }

    @MainActor
    func generateDescription() async {
        guard recipe != nil else { return }
        isGeneratingDescription = true
        autoEnrichError = nil

        do {
            let result = try await RecipesAPI.generateDescription(id: recipeId)
            if result.changed {
                await loadRecipe()
            }
        } catch is CancellationError {
        } catch {
            autoEnrichError = "Failed to generate description"
        }
        isGeneratingDescription = false
    }

    @MainActor
    func normalizeTitle() async {
        guard recipe != nil else { return }
        isNormalizingTitle = true
        autoEnrichError = nil

        do {
            let result = try await RecipesAPI.normalizeTitle(id: recipeId)
            if result.changed {
                await loadRecipe()
            }
        } catch is CancellationError {
        } catch {
            autoEnrichError = "Failed to normalize title"
        }
        isNormalizingTitle = false
    }
}
