import SwiftUI

struct AddToShoppingListSheet: View {
    let recipe: RecipeResponse
    @Binding var isPresented: Bool

    @State private var selectedIngredients: Set<Int> = []
    @State private var showingConfirmation = false

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
                                    Text(formatIngredient(ingredient))
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
        let ingredients = selectedIngredients.compactMap { index -> (name: String, amount: String?)? in
            guard index < recipe.ingredients.count else { return nil }
            let ingredient = recipe.ingredients[index]
            let amount = ingredient.measurements.first.flatMap { measurement in
                [measurement.amount, measurement.unit].compactMap { $0 }.joined(separator: " ")
            }
            return (name: ingredient.item, amount: amount?.isEmpty == true ? nil : amount)
        }

        ShoppingListStore.shared.addItemsFromRecipe(
            ingredients: ingredients,
            recipeId: recipe.id,
            recipeTitle: recipe.title
        )

        showingConfirmation = true

        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            isPresented = false
        }
    }

    private func formatIngredient(_ ingredient: Ingredient) -> String {
        var parts: [String] = []
        if let measurement = ingredient.measurements.first {
            if let amount = measurement.amount, !amount.isEmpty {
                parts.append(amount)
            }
            if let unit = measurement.unit, !unit.isEmpty {
                parts.append(unit)
            }
        }
        parts.append(ingredient.item)
        return parts.joined(separator: " ")
    }
}

// Preview requires mock data that matches generated types - skipped
