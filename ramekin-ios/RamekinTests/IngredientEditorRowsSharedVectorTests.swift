import XCTest
@testable import Ramekin

private enum VectorRow: Decodable, Equatable {
    case section(String)
    case ingredient(Ingredient)

    private enum CodingKeys: String, CodingKey {
        case section
        case ingredient
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if let name = try container.decodeIfPresent(String.self, forKey: .section) {
            self = .section(name)
        } else {
            self = .ingredient(try container.decode(Ingredient.self, forKey: .ingredient))
        }
    }

    init(_ row: IngredientEditorRow) {
        switch row {
        case .section(_, let name): self = .section(name)
        case .ingredient(let ingredient): self = .ingredient(ingredient.toIngredient(section: nil))
        }
    }

    var editorRow: IngredientEditorRow {
        switch self {
        case .section(let name): return .newSection(name)
        case .ingredient(let ingredient): return .ingredient(.from(ingredient))
        }
    }
}

private struct FromIngredientsVector: Decodable {
    let name: String
    let ingredients: [Ingredient]
    let rows: [VectorRow]
}

private struct ToIngredientsVector: Decodable {
    let name: String
    let rows: [VectorRow]
    let ingredients: [Ingredient]
}

private struct IngredientEditorRowsVectors: Decodable {
    let fromIngredients: [FromIngredientsVector]
    let toIngredients: [ToIngredientsVector]
}

final class IngredientEditorRowsSharedVectorTests: XCTestCase {
    private func loadVectors() throws -> IngredientEditorRowsVectors {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "ingredient-editor-rows", withExtension: "json")
        )
        return try JSONDecoder().decode(IngredientEditorRowsVectors.self, from: Data(contentsOf: url))
    }

    func testRowsFromIngredientsMatchesSharedVectors() throws {
        for vector in try loadVectors().fromIngredients {
            XCTAssertEqual(
                ingredientEditorRows(from: vector.ingredients).map(VectorRow.init),
                vector.rows,
                vector.name
            )
        }
    }

    func testIngredientsFromRowsMatchesSharedVectors() throws {
        for vector in try loadVectors().toIngredients {
            XCTAssertEqual(
                ingredients(from: vector.rows.map(\.editorRow)),
                vector.ingredients,
                vector.name
            )
        }
    }

    func testSectionEndIndexStopsAtNextHeading() {
        let rows: [IngredientEditorRow] = [
            .newSection("Batter"),
            .ingredient(.empty()),
            .newSection("Glaze"),
            .ingredient(.empty())
        ]
        XCTAssertEqual(ingredientSectionEndIndex(rows, sectionIndex: 0), 2)
        XCTAssertEqual(ingredientSectionEndIndex(rows, sectionIndex: 2), 4)
    }
}
