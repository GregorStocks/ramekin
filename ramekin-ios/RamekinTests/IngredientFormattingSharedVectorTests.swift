import XCTest
@testable import Ramekin

private struct IngredientFormattingVector: Decodable {
    let name: String
    let ingredient: Ingredient
    let options: IngredientFormattingOptions
    let expected: String
}

private struct IngredientFormattingOptions: Decodable {
    let scale: Double?
    let includeAlternatives: Bool
    let includeNote: Bool
    let derived: Measurement?

    private enum CodingKeys: String, CodingKey {
        case scale
        case includeAlternatives
        case includeNote
        case derived
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        scale = try container.decodeIfPresent(Double.self, forKey: .scale)
        includeAlternatives = try container.decodeIfPresent(
            Bool.self,
            forKey: .includeAlternatives
        ) ?? false
        includeNote = try container.decodeIfPresent(Bool.self, forKey: .includeNote) ?? false
        derived = try container.decodeIfPresent(Measurement.self, forKey: .derived)
    }
}

final class IngredientFormattingSharedVectorTests: XCTestCase {
    func testIngredientFormattingMatchesSharedVectors() throws {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "ingredient-formatting", withExtension: "json")
        )
        let vectors = try JSONDecoder().decode(
            [IngredientFormattingVector].self,
            from: Data(contentsOf: url)
        )

        for vector in vectors {
            XCTAssertEqual(
                vector.ingredient.formatted(
                    scale: vector.options.scale ?? 1,
                    includeAlternatives: vector.options.includeAlternatives,
                    includeNote: vector.options.includeNote,
                    derived: vector.options.derived
                ),
                vector.expected,
                vector.name
            )
        }
    }
}

final class DerivedMeasurementLookupTests: XCTestCase {
    func testFindsDerivedGramsByIngredientIndex() {
        let derived = [
            DerivedMeasurement(amount: "120", ingredientIndex: 0, unit: "g"),
            DerivedMeasurement(amount: "227", ingredientIndex: 2, unit: "g")
        ]

        XCTAssertEqual(derived.measurement(forIngredientAt: 2), Measurement(amount: "227", unit: "g"))
        XCTAssertNil(derived.measurement(forIngredientAt: 1))
    }
}
