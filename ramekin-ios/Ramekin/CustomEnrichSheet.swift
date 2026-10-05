import SwiftUI

struct CustomEnrichSheet: View {
    let recipe: RecipeResponse
    @Binding var isPresented: Bool
    let onResult: (RecipeContent) -> Void

    @State private var instruction = ""
    @State private var isLoading = false
    @State private var error: String?
    @State private var submitTask: Task<Void, Never>?

    var body: some View {
        NavigationStack {
            Form {
                Section("What would you like to change?") {
                    TextField("e.g., make this vegan, double the servings…", text: $instruction, axis: .vertical)
                        .lineLimit(2...5)
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
                            ProgressView("Customizing recipe…")
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
                        submitTask?.cancel()
                        isPresented = false
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Submit") {
                        submitTask = Task { await submit() }
                    }
                    .disabled(instruction.trimmingCharacters(in: .whitespaces).isEmpty || isLoading)
                }
            }
            .interactiveDismissDisabled(isLoading)
        }
        // Closing the sheet abandons the request, so a late result must not
        // pop open the review sheet.
        .onDisappear { submitTask?.cancel() }
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
            guard !Task.isCancelled else { return }
            await MainActor.run {
                onResult(result)
                isPresented = false
            }
        } catch {
            guard !Task.isCancelled else { return }
            await MainActor.run {
                self.error = error.localizedDescription
                isLoading = false
            }
        }
    }
}
