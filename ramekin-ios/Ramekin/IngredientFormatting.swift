import Foundation

extension Ingredient {
    func formatted(
        includeAlternatives: Bool = false,
        includeNote: Bool = false
    ) -> String {
        var parts: [String] = []

        if let measurement = measurements.first {
            let primary = [measurement.amount, measurement.unit]
                .compactMap { value -> String? in
                    guard let value else {
                        return nil
                    }

                    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
                    return trimmed.isEmpty ? nil : trimmed
                }

            if !primary.isEmpty {
                parts.append(primary.joined(separator: " "))
            }
        }

        if includeAlternatives {
            let alternatives = measurements.dropFirst().compactMap { measurement -> String? in
                let values = [measurement.amount, measurement.unit]
                    .compactMap { value -> String? in
                        guard let value else {
                            return nil
                        }

                        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
                        return trimmed.isEmpty ? nil : trimmed
                    }

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
}
