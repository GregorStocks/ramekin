import Foundation

enum RecipeScaleSupport {
    static let presets: [(value: Double, label: String)] = [
        (0.25, "1/4x"),
        (0.5, "1/2x"),
        (1, "1x"),
        (2, "2x"),
        (3, "3x")
    ]

    static func scaleAmount(_ amount: String?, by factor: Double) -> String {
        guard let amount, !amount.isEmpty else {
            return amount ?? ""
        }
        guard factor.isFinite, factor > 0, factor != 1 else {
            return amount
        }
        guard let parsed = parseAmount(amount) else {
            return amount
        }

        return formatScaled(parsed * factor)
    }

    static func formatScaleLabel(_ value: Double) -> String {
        if abs(value - 0.25) < 0.000001 {
            return "1/4x"
        }
        if abs(value - 0.5) < 0.000001 {
            return "1/2x"
        }
        if abs(value - 1.0 / 3.0) < 0.000001 {
            return "1/3x"
        }
        if abs(value - 2.0 / 3.0) < 0.000001 {
            return "2/3x"
        }

        let formatter = NumberFormatter()
        formatter.minimumFractionDigits = 0
        formatter.maximumFractionDigits = 2
        formatter.numberStyle = .decimal
        return "\(formatter.string(from: NSNumber(value: value)) ?? String(value))x"
    }

    static func parseDecimal(_ raw: String) -> Double? {
        let amount = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !amount.isEmpty else {
            return nil
        }

        if let value = Double(amount) {
            return value
        }

        let formatter = NumberFormatter()
        formatter.locale = .current
        formatter.numberStyle = .decimal
        if let value = formatter.number(from: amount)?.doubleValue {
            return value
        }

        guard amount.contains(","),
              !amount.contains("."),
              amount.filter({ $0 == "," }).count == 1 else {
            return nil
        }

        return Double(amount.replacingOccurrences(of: ",", with: "."))
    }

    private static func parseAmount(_ raw: String) -> Double? {
        let amount = normalizeFractions(raw).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !amount.isEmpty else { return nil }

        let mixedParts = amount.split(separator: " ", omittingEmptySubsequences: true)
        if mixedParts.count == 2,
           let whole = parseDecimal(String(mixedParts[0])),
           let fraction = parseFraction(String(mixedParts[1])) {
            return whole + fraction
        }

        if let fraction = parseFraction(amount) {
            return fraction
        }

        guard amount.range(
            of: #"^\d+([\.,]\d+)?$|^[\.,]\d+$"#,
            options: .regularExpression
        ) != nil else {
            return nil
        }

        return parseDecimal(amount)
    }

    private static func parseFraction(_ amount: String) -> Double? {
        let parts = amount.split(separator: "/", omittingEmptySubsequences: false)
        guard parts.count == 2,
              let numerator = parseDecimal(String(parts[0])),
              let denominator = parseDecimal(String(parts[1])),
              denominator != 0 else {
            return nil
        }

        return numerator / denominator
    }

    private static func normalizeFractions(_ input: String) -> String {
        let fractions: [Character: String] = [
            "½": "1/2",
            "⅓": "1/3",
            "⅔": "2/3",
            "¼": "1/4",
            "¾": "3/4",
            "⅕": "1/5",
            "⅖": "2/5",
            "⅗": "3/5",
            "⅘": "4/5",
            "⅙": "1/6",
            "⅚": "5/6",
            "⅛": "1/8",
            "⅜": "3/8",
            "⅝": "5/8",
            "⅞": "7/8"
        ]

        var output = ""
        for character in input {
            if let replacement = fractions[character] {
                if let last = output.last, last.isNumber {
                    output.append(" ")
                }
                output.append(replacement)
            } else {
                output.append(character)
            }
        }
        return output
    }

    private static func formatScaled(_ value: Double) -> String {
        let rounded = value.rounded()
        if abs(value - rounded) < 0.000001 {
            return String(Int(rounded))
        }

        for denominator in [2.0, 3.0, 4.0, 6.0, 8.0]
            where abs(value - (1.0 / denominator)) < 0.000001 {
            return "1/\(Int(denominator))"
        }

        let formatter = NumberFormatter()
        formatter.minimumFractionDigits = 0
        formatter.maximumFractionDigits = 2
        formatter.numberStyle = .decimal
        return formatter.string(from: NSNumber(value: value)) ?? String(value)
    }
}
