import Foundation

extension Ingredient {
    func formatted(
        scale: Double = 1,
        includeAlternatives: Bool = false,
        includeNote: Bool = false,
        derived: Measurement? = nil
    ) -> String {
        var parts: [String] = []
        let primary = measurements.first.flatMap { Self.formattedMeasurement($0, scale: scale) }

        // Derived grams the server computed lead the line, marked approximate
        // ("~120 g"), and push the stored primary into the alternatives.
        // Grams without an amount say nothing.
        var derivedLead: String?
        if includeAlternatives, let derived,
           Self.trimmedValue(RecipeScaleSupport.scaleAmount(derived.amount, by: scale)) != nil,
           let formatted = Self.formattedMeasurement(derived, scale: scale) {
            derivedLead = "~\(formatted)"
        }

        if let lead = derivedLead ?? primary {
            parts.append(lead)
        }

        if includeAlternatives {
            // A per-item size ("10 oz each") stays the same however many
            // items the recipe calls for.
            var alternatives = measurements.dropFirst().compactMap { measurement in
                Self.formattedMeasurement(measurement, scale: Self.isPerItem(measurement) ? 1 : scale)
            }
            if derivedLead != nil, let primary {
                alternatives.insert(primary, at: 0)
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

    private static func isPerItem(_ measurement: Measurement) -> Bool {
        guard let unit = trimmedValue(measurement.unit)?.lowercased() else {
            return false
        }
        return unit.hasSuffix(" each")
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
