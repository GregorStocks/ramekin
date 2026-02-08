import SwiftUI

struct CustomEnrichSheet: View {
    let recipe: RecipeResponse
    @Binding var isPresented: Bool
    let onResult: (RecipeContent) -> Void

    @State private var instruction = ""
    @State private var isLoading = false
    @State private var error: String?

    var body: some View {
        NavigationStack {
            Form {
                Section("What would you like to change?") {
                    TextField("e.g., make this vegan, double the servings...", text: $instruction)
                }

                if let error {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                    }
                }

                if isLoading {
                    Section {
                        HStack {
                            Spacer()
                            ProgressView("Customizing recipe...")
                            Spacer()
                        }
                    }
                }
            }
            .navigationTitle("Customize with AI")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        isPresented = false
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Submit") {
                        Task { await submit() }
                    }
                    .disabled(instruction.trimmingCharacters(in: .whitespaces).isEmpty || isLoading)
                }
            }
        }
    }

    private func submit() async {
        isLoading = true
        error = nil

        do {
            let recipeContent = RecipeContent(
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
            let request = CustomEnrichRequest(instruction: instruction, recipe: recipeContent)
            let result = try await EnrichAPI.customEnrichRecipe(customEnrichRequest: request)
            await MainActor.run {
                onResult(result)
                isPresented = false
            }
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
                isLoading = false
            }
        }
    }
}
