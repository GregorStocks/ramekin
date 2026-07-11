import XCTest
@testable import Ramekin

final class IngredientFormattingSharedVectorTests: XCTestCase {
    private struct Vector: Decodable {
        let name: String
        let ingredient: Ingredient
        let options: Options
        let expected: String
    }

    private struct Options: Decodable {
        let scale: Double?
        let includeAlternatives: Bool
        let includeNote: Bool

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

    func testIngredientFormattingMatchesSharedVectors() throws {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "ingredient-formatting", withExtension: "json")
        )
        let vectors = try JSONDecoder().decode([Vector].self, from: Data(contentsOf: url))

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
