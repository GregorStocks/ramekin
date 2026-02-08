import SwiftUI

struct EnrichPreviewSheet: View {
    let original: RecipeResponse
    let modified: RecipeContent
    let onApply: () -> Void
    let onCancel: () -> Void

    @State private var isApplying = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if original.title != modified.title {
                        fieldDiff("Title", old: original.title, new: modified.title)
                    }

                    if original.description != modified.description {
                        fieldDiff("Description", old: original.description ?? "", new: modified.description ?? "")
                    }

                    if formatIngredients(original.ingredients) != formatIngredients(modified.ingredients) {
                        fieldDiff("Ingredients", old: formatIngredients(original.ingredients), new: formatIngredients(modified.ingredients))
                    }

                    if original.instructions != modified.instructions {
                        fieldDiff("Instructions", old: original.instructions, new: modified.instructions)
                    }

                    if original.tags.joined(separator: ", ") != (modified.tags ?? []).joined(separator: ", ") {
                        fieldDiff("Tags", old: original.tags.joined(separator: ", "), new: (modified.tags ?? []).joined(separator: ", "))
                    }

                    if original.notes != modified.notes {
                        fieldDiff("Notes", old: original.notes ?? "", new: modified.notes ?? "")
                    }

                    if original.servings != modified.servings {
                        fieldDiff("Servings", old: original.servings ?? "", new: modified.servings ?? "")
                    }

                    if original.prepTime != modified.prepTime {
                        fieldDiff("Prep Time", old: original.prepTime ?? "", new: modified.prepTime ?? "")
                    }

                    if original.cookTime != modified.cookTime {
                        fieldDiff("Cook Time", old: original.cookTime ?? "", new: modified.cookTime ?? "")
                    }

                    if original.totalTime != modified.totalTime {
                        fieldDiff("Total Time", old: original.totalTime ?? "", new: modified.totalTime ?? "")
                    }

                    if original.difficulty != modified.difficulty {
                        fieldDiff("Difficulty", old: original.difficulty ?? "", new: modified.difficulty ?? "")
                    }

                    if original.nutritionalInfo != modified.nutritionalInfo {
                        fieldDiff("Nutritional Info", old: original.nutritionalInfo ?? "", new: modified.nutritionalInfo ?? "")
                    }

                    if !hasAnyChanges {
                        Text("No changes suggested.")
                            .foregroundColor(.secondary)
                            .frame(maxWidth: .infinity, alignment: .center)
                            .padding(.top, 40)
                    }
                }
                .padding()
            }
            .navigationTitle("Review Changes")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: onCancel)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Apply Changes") {
                        isApplying = true
                        onApply()
                    }
                    .disabled(isApplying || !hasAnyChanges)
                }
            }
        }
    }

    private var hasAnyChanges: Bool {
        let textChanged: Bool = original.title != modified.title
            || original.description != modified.description
            || original.instructions != modified.instructions
        let metaChanged: Bool = original.notes != modified.notes
            || original.servings != modified.servings
            || original.prepTime != modified.prepTime
        let timeChanged: Bool = original.cookTime != modified.cookTime
            || original.totalTime != modified.totalTime
            || original.difficulty != modified.difficulty
        let listChanged: Bool = original.nutritionalInfo != modified.nutritionalInfo
            || formatIngredients(original.ingredients) != formatIngredients(modified.ingredients)
            || original.tags.joined(separator: ", ") != (modified.tags ?? []).joined(separator: ", ")
        return textChanged || metaChanged || timeChanged || listChanged
    }

    private func fieldDiff(_ label: String, old: String, new: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(label)
                .font(.headline)

            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .top) {
                    Text("Before:")
                        .font(.caption)
                        .fontWeight(.bold)
                        .foregroundColor(.red)
                        .frame(width: 50, alignment: .leading)
                    Text(old.isEmpty ? "(empty)" : old)
                        .font(.body)
                        .foregroundColor(old.isEmpty ? .secondary : .primary)
                }

                HStack(alignment: .top) {
                    Text("After:")
                        .font(.caption)
                        .fontWeight(.bold)
                        .foregroundColor(.green)
                        .frame(width: 50, alignment: .leading)
                    Text(new.isEmpty ? "(empty)" : new)
                        .font(.body)
                        .foregroundColor(new.isEmpty ? .secondary : .primary)
                }
            }
            .padding(12)
            .background(Color(.systemGray6))
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }

    private func formatIngredients(_ ingredients: [Ingredient]) -> String {
        ingredients.map { ingredient in
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
        }.joined(separator: "\n")
    }
}
