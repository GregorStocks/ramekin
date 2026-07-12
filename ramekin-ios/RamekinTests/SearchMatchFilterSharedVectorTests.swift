import XCTest
@testable import Ramekin

private struct FixtureFile: Decodable {
    let recipes: [FixtureRecipe]
    let cases: [FixtureCase]
}

private struct FixtureMeasurement: Decodable {
    let amount: String?
    let unit: String?
}

private struct FixtureIngredient: Decodable {
    let item: String
    let measurements: [FixtureMeasurement]
    let note: String?
    let section: String?
}

private struct FixtureRecipe: Decodable {
    let id: String
    let title: String
    let description: String?
    let tags: [String]
    let ingredients: [FixtureIngredient]
    let ingredientMatchText: String
    let instructions: String
    let notes: String?
    let rating: Int?
    let hasPhoto: Bool
    let createdAt: String
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, title, description, tags, ingredients, instructions, notes, rating
        case ingredientMatchText = "ingredient_match_text"
        case hasPhoto = "has_photo"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

private struct FixtureCase: Decodable {
    let name: String
    let query: String
    let sortBy: String?
    let sortDir: String?
    let expectedIds: [String]

    enum CodingKeys: String, CodingKey {
        case name, query
        case sortBy = "sort_by"
        case sortDir = "sort_dir"
        case expectedIds = "expected_ids"
    }
}

/// Consumes shared-test-vectors/search-match-filter.json — the end-to-end
/// search contract: raw query strings plus recipe documents in, matched ids
/// in final display order out. The server side replays the same file through
/// the real API and database (tests/test_search_match_filter_vectors.py);
/// this test replays it through the complete Swift local pipeline: parsing,
/// normalization, membership, scoring, and ordering.
final class SearchMatchFilterSharedVectorTests: XCTestCase {

    func testSearchMatchFilterVectors() throws {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "search-match-filter", withExtension: "json")
        )
        let fixture = try JSONDecoder().decode(FixtureFile.self, from: Data(contentsOf: url))

        // The wire formatter the app decodes sync timestamps with.
        let timestampFormatter = OpenISO8601DateFormatter()
        var slugsByID: [UUID: String] = [:]
        let documents = try fixture.recipes.map { recipe -> CachedRecipeSearchDocument in
            let id = UUID()
            slugsByID[id] = recipe.id
            let summary = RecipeSummary(
                createdAt: try XCTUnwrap(timestampFormatter.date(from: recipe.createdAt)),
                description: recipe.description,
                id: id,
                rating: recipe.rating,
                tags: recipe.tags,
                thumbnailPhotoId: recipe.hasPhoto ? UUID() : nil,
                title: recipe.title,
                updatedAt: try XCTUnwrap(timestampFormatter.date(from: recipe.updatedAt))
            )
            return CachedRecipeSearchDocument(
                summary: summary,
                ingredients: recipe.ingredients.map { ingredient in
                    Ingredient(
                        item: ingredient.item,
                        measurements: ingredient.measurements.map {
                            Measurement(amount: $0.amount, unit: $0.unit)
                        },
                        note: ingredient.note,
                        section: ingredient.section
                    )
                },
                ingredientMatchText: recipe.ingredientMatchText,
                instructions: recipe.instructions,
                notes: recipe.notes
            )
        }

        for testCase in fixture.cases {
            let sortBy = try testCase.sortBy.map { raw in
                try XCTUnwrap(SortBy(rawValue: raw), "case '\(testCase.name)': bad sort_by '\(raw)'")
            }
            let sortDir = try testCase.sortDir.map { raw in
                try XCTUnwrap(Direction(rawValue: raw), "case '\(testCase.name)': bad sort_dir '\(raw)'")
            }

            let results = RecipeSearchSupport.execute(
                documents: documents,
                parsed: RecipeSearchSupport.parse(testCase.query),
                sortBy: sortBy,
                sortDir: sortDir
            )

            XCTAssertEqual(
                results.map { slugsByID[$0.id]! },
                testCase.expectedIds,
                "case '\(testCase.name)' (query: \(testCase.query))"
            )
        }
    }
}
