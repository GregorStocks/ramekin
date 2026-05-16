import Foundation

extension Ingredient {
    func formatted(
        scale: Double = 1,
        includeAlternatives: Bool = false,
        includeNote: Bool = false
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
            let alternatives = measurements.dropFirst().compactMap { measurement -> String? in
                let amount = RecipeScaleSupport.scaleAmount(measurement.amount, by: scale)
                let values = [amount, measurement.unit]
                    .compactMap(Self.trimmedValue)

                guard !values.isEmpty else {
                    return nil
                }

                return values.joined(separator: " ")
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

    private static func trimmedValue(_ value: String?) -> String? {
        guard let value else {
            return nil
        }

        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
