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

    private enum CodingKeys: String, CodingKey {
        case scale
        case includeAlternatives
        case includeNote
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        scale = try container.decodeIfPresent(Double.self, forKey: .scale)
        includeAlternatives = try container.decodeIfPresent(
            Bool.self,
            forKey: .includeAlternatives
        ) ?? false
        includeNote = try container.decodeIfPresent(Bool.self, forKey: .includeNote) ?? false
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
                    includeNote: vector.options.includeNote
                ),
                vector.expected,
                vector.name
            )
        }
    }
}
