import Foundation

// MARK: - Editable Models

struct EditableIngredient: Equatable, Identifiable {
    let id = UUID()
    var item: String
    var measurements: [EditableMeasurement]
    var note: String
    var section: String
    var raw: String?

    static func empty(section: String = "") -> EditableIngredient {
        EditableIngredient(
            item: "",
            measurements: [EditableMeasurement()],
            note: "",
            section: section
        )
    }

    func toIngredient() -> Ingredient {
        Ingredient(
            item: item,
            measurements: measurements.map { $0.toMeasurement() },
            note: note.isEmpty ? nil : note,
            raw: raw,
            section: section.isEmpty ? nil : section
        )
    }

    static func from(_ ingredient: Ingredient) -> EditableIngredient {
        EditableIngredient(
            item: ingredient.item,
            measurements: ingredient.measurements.map { EditableMeasurement.from($0) },
            note: ingredient.note ?? "",
            section: ingredient.section ?? "",
            raw: ingredient.raw
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

// MARK: - Ingredient Grouping

struct IngredientGroup {
    let section: String
    let indices: [Int]
}

func groupIngredientsBySection(_ ingredients: [EditableIngredient]) -> [IngredientGroup] {
    groupConsecutiveItemsBySection(Array(ingredients.enumerated())) { indexedIngredient in
        indexedIngredient.element.section
    }.map { group in
        IngredientGroup(
            section: group.section ?? "",
            indices: group.items.map(\.offset)
        )
    }
}
