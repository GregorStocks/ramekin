import SwiftUI

/// One recipe field that differs between two contents.
struct RecipeContentFieldDiff: Identifiable, Equatable {
    let label: String
    let before: String
    let after: String

    var id: String { label }

    /// Ingredients as their parsed parts, one per line, so a re-parse that
    /// reads the same ("1 cup water" as the item vs. amount 1, unit cup, item
    /// water) still shows as a change.
    static func formatIngredientStructure(_ ingredients: [Ingredient]) -> String {
        ingredients.map { ingredient in
            let section = ingredient.section.map { $0.isEmpty ? "" : "[\($0)] " } ?? ""
            let measurements = ingredient.measurements
                .map { "\($0.amount ?? "") \($0.unit ?? "")".trimmingCharacters(in: .whitespaces) }
                .joined(separator: " / ")
            let note = ingredient.note.map { $0.isEmpty ? "" : " | \($0)" } ?? ""
            return "\(section)\(ingredient.item) | \(measurements)\(note)"
        }
        .joined(separator: "\n")
    }

    /// The fields that differ between `old` and `new`, in reading order.
    /// "Ingredients (parsed)" appears only when the ingredients read the same
    /// but were parsed differently.
    ///
    /// Mirrors ramekin-ui/src/utils/recipeContentChanges.ts; pinned by
    /// shared-test-vectors/normalize-selection.json (proposal_changes).
    static func diffs(old: RecipeContent, new: RecipeContent) -> [RecipeContentFieldDiff] {
        let fields: [(String, (RecipeContent) -> String)] = [
            ("Title", { $0.title }),
            ("Description", { $0.description ?? "" }),
            ("Ingredients", { RecipeVersionSupport.formatIngredients($0.ingredients) }),
            ("Ingredients (parsed)", { formatIngredientStructure($0.ingredients) }),
            ("Instructions", { $0.instructions }),
            ("Tags", { RecipeVersionSupport.formatTags($0.tags ?? []) }),
            ("Notes", { $0.notes ?? "" }),
            ("Prep Time", { $0.prepTime ?? "" }),
            ("Cook Time", { $0.cookTime ?? "" }),
            ("Total Time", { $0.totalTime ?? "" }),
            ("Servings", { $0.servings ?? "" }),
            ("Difficulty", { $0.difficulty ?? "" }),
            ("Rating", { $0.rating.map(String.init) ?? "" }),
            ("Nutritional Info", { $0.nutritionalInfo ?? "" }),
            ("Source", { $0.sourceName ?? "" }),
            ("Source URL", { $0.sourceUrl ?? "" })
        ]
        let changes: [RecipeContentFieldDiff] = fields.compactMap { label, text in
            let before = text(old)
            let after = text(new)
            return before == after ? nil : RecipeContentFieldDiff(label: label, before: before, after: after)
        }
        guard changes.contains(where: { $0.label == "Ingredients" }) else { return changes }
        return changes.filter { $0.label != "Ingredients (parsed)" }
    }

    /// Whether accepting `proposed` over `base` would change anything.
    static func changed(base: NormalizedRecipe, proposed: NormalizedRecipe) -> Bool {
        !diffs(old: base.content, new: proposed.content).isEmpty || base.photoIds != proposed.photoIds
    }
}

/// Before/after of each field that differs between two recipe contents.
struct RecipeContentDiffView: View {
    let diffs: [RecipeContentFieldDiff]
    let emptyMessage: String

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if diffs.isEmpty {
                Text(emptyMessage)
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
            } else {
                ForEach(diffs) { diff in
                    fieldDiff(diff)
                }
            }
        }
    }

    private func fieldDiff(_ diff: RecipeContentFieldDiff) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(diff.label)
                .font(.headline)

            VStack(alignment: .leading, spacing: 4) {
                side("Before:", text: diff.before, color: .red)
                side("After:", text: diff.after, color: .green)
            }
            .padding(12)
            .background(Color(.systemGray6))
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }

    private func side(_ title: String, text: String, color: Color) -> some View {
        HStack(alignment: .top) {
            Text(title)
                .font(.caption)
                .fontWeight(.bold)
                .foregroundColor(color)
                .frame(minWidth: 50, alignment: .leading)
                .fixedSize()
            Text(text.isEmpty ? "(empty)" : text)
                .font(.body)
                .foregroundColor(text.isEmpty ? .secondary : .primary)
        }
    }
}
