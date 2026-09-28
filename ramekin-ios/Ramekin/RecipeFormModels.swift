import Foundation

// MARK: - Editable Models

struct EditableIngredient: Equatable, Identifiable {
    let id = UUID()
    var item: String
    var measurements: [EditableMeasurement]
    var note: String

    static func empty() -> EditableIngredient {
        EditableIngredient(
            item: "",
            measurements: [EditableMeasurement()],
            note: ""
        )
    }

    func toIngredient(section: String?) -> Ingredient {
        Ingredient(
            item: item,
            measurements: measurements.map { $0.toMeasurement() },
            note: note.isEmpty ? nil : note,
            section: section
        )
    }

    static func from(_ ingredient: Ingredient) -> EditableIngredient {
        EditableIngredient(
            item: ingredient.item,
            measurements: ingredient.measurements.map { EditableMeasurement.from($0) },
            note: ingredient.note ?? ""
        )
    }
}

struct EditableMeasurement: Equatable, Identifiable {
    let id = UUID()
    var amount: String
    var unit: String

    init(amount: String = "", unit: String = "") {
        self.amount = amount
        self.unit = unit
    }

    func toMeasurement() -> Measurement {
        Measurement(
            amount: amount.isEmpty ? nil : amount,
            unit: unit.isEmpty ? nil : unit
        )
    }

    static func from(_ measurement: Measurement) -> EditableMeasurement {
        EditableMeasurement(
            amount: measurement.amount ?? "",
            unit: measurement.unit ?? ""
        )
    }
}

// MARK: - Form Mode

enum RecipeFormMode: Equatable {
    case create
    case edit(recipeId: UUID)
}

// MARK: - Ingredient Editor Rows

/// The recipe form edits ingredients as one flat list of rows, where a section
/// row starts a named section that runs until the next section row. Keeping
/// sections as their own rows lets a section exist while it is empty and lets
/// an ingredient move between sections with a single reorder.
///
/// Mirrors ramekin-ui/src/utils/ingredientEditorRows.ts; both are pinned by
/// shared-test-vectors/ingredient-editor-rows.json.
enum IngredientEditorRow: Identifiable, Equatable {
    case section(id: UUID, name: String)
    case ingredient(EditableIngredient)

    static func newSection(_ name: String) -> IngredientEditorRow {
        .section(id: UUID(), name: name)
    }

    var id: UUID {
        switch self {
        case .section(let id, _): return id
        case .ingredient(let ingredient): return ingredient.id
        }
    }

    var isSection: Bool {
        if case .section = self { return true }
        return false
    }
}

/// A section name with surrounding whitespace removed; blank means none.
private func normalizedSectionName(_ name: String?) -> String? {
    guard let trimmed = name?.trimmingCharacters(in: .whitespacesAndNewlines),
          !trimmed.isEmpty else {
        return nil
    }
    return trimmed
}

/// Build editor rows, inserting a section row wherever the section changes. A
/// return to no section after a named one gets a blank-named section row so
/// those ingredients don't fall into the section above.
func ingredientEditorRows(from ingredients: [Ingredient]) -> [IngredientEditorRow] {
    var rows: [IngredientEditorRow] = []
    var currentSection: String?
    for ingredient in ingredients {
        let section = normalizedSectionName(ingredient.section)
        if section != currentSection {
            currentSection = section
            rows.append(.newSection(section ?? ""))
        }
        rows.append(.ingredient(.from(ingredient)))
    }
    return rows
}

/// Flatten editor rows back into ingredients. Each ingredient takes the trimmed
/// name of the nearest section row above it; a blank name means no section.
func ingredients(from rows: [IngredientEditorRow]) -> [Ingredient] {
    var result: [Ingredient] = []
    var currentSection: String?
    for row in rows {
        switch row {
        case .section(_, let name):
            currentSection = normalizedSectionName(name)
        case .ingredient(let ingredient):
            result.append(ingredient.toIngredient(section: currentSection))
        }
    }
    return result
}

/// Index just past the last row of the section whose heading is at `sectionIndex`.
func ingredientSectionEndIndex(_ rows: [IngredientEditorRow], sectionIndex: Int) -> Int {
    var end = sectionIndex + 1
    while end < rows.count, !rows[end].isSection {
        end += 1
    }
    return end
}

/// Ids of ingredient rows that sit under a named section heading.
func sectionedIngredientIds(_ rows: [IngredientEditorRow]) -> Set<UUID> {
    var ids = Set<UUID>()
    var inSection = false
    for row in rows {
        switch row {
        case .section(_, let name):
            inSection = normalizedSectionName(name) != nil
        case .ingredient(let ingredient):
            if inSection { ids.insert(ingredient.id) }
        }
    }
    return ids
}
