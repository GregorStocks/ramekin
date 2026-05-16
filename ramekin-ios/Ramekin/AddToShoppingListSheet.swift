import SwiftUI

enum AddToShoppingListSheetSupport {
    static func ingredientsForShoppingList(
        recipe: RecipeResponse,
        selectedIngredients: Set<Int>,
        scale: Double
    ) -> [(name: String, amount: String?)] {
        recipe.ingredients.enumerated().compactMap { index, ingredient in
            guard selectedIngredients.contains(index) else {
                return nil
            }

            return (
                name: ingredient.item,
                amount: formattedAmount(ingredient, scale: scale)
            )
        }
    }

    static func formattedAmount(_ ingredient: Ingredient, scale: Double) -> String? {
        guard let measurement = ingredient.measurements.first else {
            return nil
        }

        let scaledAmount = RecipeScaleSupport.scaleAmount(measurement.amount, by: scale)
        let amount = [scaledAmount, measurement.unit]
            .compactMap { value -> String? in
                guard let value else {
                    return nil
                }

                let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
                return trimmed.isEmpty ? nil : trimmed
            }
            .joined(separator: " ")

        return amount.isEmpty ? nil : amount
    }
}

struct AddToShoppingListSheet: View {
    let recipe: RecipeResponse
    let scale: Double
    @Binding var isPresented: Bool

    @State private var selectedIngredients: Set<Int> = []
    @State private var showingConfirmation = false
    @State private var error: String?

    var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(Array(recipe.ingredients.enumerated()), id: \.offset) { index, ingredient in
                        Button {
                            if selectedIngredients.contains(index) {
                                selectedIngredients.remove(index)
                            } else {
                                selectedIngredients.insert(index)
                            }
                        } label: {
                            HStack {
                                Image(systemName: selectedIngredients.contains(index) ? "checkmark.circle.fill" : "circle")
                                    .foregroundColor(selectedIngredients.contains(index) ? .orange : .secondary)
                                    .font(.title3)

                                VStack(alignment: .leading, spacing: 2) {
                                    Text(ingredient.formatted(scale: scale))
                                        .foregroundColor(.primary)

                                    if let note = ingredient.note, !note.isEmpty {
                                        Text(note)
                                            .font(.caption)
                                            .foregroundColor(.secondary)
                                            .italic()
                                    }
                                }
                            }
                        }
                        .buttonStyle(.plain)
                    }
                } header: {
                    HStack {
                        Text("Select ingredients to add")
                        Spacer()
                        Button(allSelected ? "Deselect All" : "Select All") {
                            if allSelected {
                                selectedIngredients.removeAll()
                            } else {
                                selectedIngredients = Set(0..<recipe.ingredients.count)
                            }
                        }
                        .font(.caption)
                    }
                }

                if let error {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Add to Shopping List")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        isPresented = false
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add \(selectedIngredients.count)") {
                        addToShoppingList()
                    }
                    .disabled(selectedIngredients.isEmpty)
                }
            }
            .onAppear {
                // Select all by default
                selectedIngredients = Set(0..<recipe.ingredients.count)
            }
            .overlay {
                if showingConfirmation {
                    confirmationOverlay
                }
            }
        }
    }

    private var allSelected: Bool {
        selectedIngredients.count == recipe.ingredients.count
    }

    private var confirmationOverlay: some View {
        VStack(spacing: 12) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 50))
                .foregroundColor(.green)
            Text("Added \(selectedIngredients.count) items")
                .font(.headline)
        }
        .padding(30)
        .background(.regularMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }

    private func addToShoppingList() {
        error = nil

        let ingredients = AddToShoppingListSheetSupport.ingredientsForShoppingList(
            recipe: recipe,
            selectedIngredients: selectedIngredients,
            scale: scale
        )

        do {
            try ShoppingListStore.shared.addItemsFromRecipe(
                ingredients: ingredients,
                recipeId: recipe.id,
                recipeTitle: recipe.title
            )

            showingConfirmation = true

            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                isPresented = false
            }
        } catch {
            self.error = error.localizedDescription
        }
    }
}

// Preview requires mock data that matches generated types - skipped
