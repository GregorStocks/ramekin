import Foundation

extension Ingredient {
    func formatted(
        scale: Double = 1,
        includeAlternatives: Bool = false,
        includeNote: Bool = false,
        derived: Measurement? = nil
    ) -> String {
        var parts: [String] = []

        if let measurement = measurements.first {
            let amount = RecipeScaleSupport.scaleAmount(measurement.amount, by: scale)
            let primary = [amount, measurement.unit]
                .compactMap(Self.trimmedValue)

            if !primary.isEmpty {
                parts.append(primary.joined(separator: " "))
            }
        }

        if includeAlternatives {
            // Derived grams the server computed come after the stored
            // alternatives, marked approximate ("~120 g").
            var alternatives = measurements.dropFirst().compactMap { measurement in
                Self.formattedMeasurement(measurement, scale: scale)
            }
            if let derived, let formatted = Self.formattedMeasurement(derived, scale: scale) {
                alternatives.append("~\(formatted)")
            }

            if !alternatives.isEmpty {
                parts.append("(\(alternatives.joined(separator: ", ")))")
            }
        }

        parts.append(item)

        if includeNote, let note {
            let trimmedNote = note.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmedNote.isEmpty {
                parts.append("(\(trimmedNote))")
            }
        }

        return parts.joined(separator: " ")
    }

    private static func formattedMeasurement(_ measurement: Measurement, scale: Double) -> String? {
        let amount = RecipeScaleSupport.scaleAmount(measurement.amount, by: scale)
        let values = [amount, measurement.unit].compactMap(Self.trimmedValue)
        return values.isEmpty ? nil : values.joined(separator: " ")
    }

    private static func trimmedValue(_ value: String?) -> String? {
        guard let value else {
            return nil
        }

        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

extension Array where Element == DerivedMeasurement {
    /// The derived grams the server computed for the ingredient at `index`.
    func measurement(forIngredientAt index: Int) -> Measurement? {
        first { $0.ingredientIndex == index }.map { Measurement(amount: $0.amount, unit: $0.unit) }
    }
}
