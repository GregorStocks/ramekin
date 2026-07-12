import XCTest
@testable import Ramekin

private struct FixtureFile: Decodable {
    let recipes: [FixtureRecipe]
    let cases: [FixtureCase]
}

private struct FixtureRecipe: Decodable {
    let id: String
    let title: String
    let description: String?
    let tags: [String]
    let ingredients: [String]
    let instructions: String
    let notes: String?
}

private struct FixtureCase: Decodable {
    let name: String
    let tokens: [String]
    let expectedOrder: [String]
    let zeroScore: [String]

    enum CodingKeys: String, CodingKey {
        case name
        case tokens
        case expectedOrder = "expected_order"
        case zeroScore = "zero_score"
    }
}

/// Consumes shared-test-vectors/search-ranking.json — the same corpus the
/// canonical Rust scorer is pinned to (ramekin-core/tests/
/// search_ranking_tests.rs) — and requires identical rankings from the Swift
/// mirror: strictly decreasing scores down each expected order, and exact
/// zeros where the server scores zero.
final class SearchRankingSharedVectorTests: XCTestCase {

    func testSearchRankingVectors() throws {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "search-ranking", withExtension: "json")
        )
        let fixture = try JSONDecoder().decode(FixtureFile.self, from: Data(contentsOf: url))
        let recipesByID = Dictionary(uniqueKeysWithValues: fixture.recipes.map { ($0.id, $0) })

        for testCase in fixture.cases {
            let scores = try testCase.expectedOrder.map { id -> (String, UInt32) in
                let recipe = try XCTUnwrap(recipesByID[id], "case '\(testCase.name)': unknown id '\(id)'")
                return (id, score(recipe, tokens: testCase.tokens))
            }

            for (higher, lower) in zip(scores, scores.dropFirst()) {
                XCTAssertGreaterThan(
                    higher.1,
                    lower.1,
                    "case '\(testCase.name)': expected '\(higher.0)' to outrank '\(lower.0)'"
                )
            }
            if let last = scores.last {
                XCTAssertGreaterThan(
                    last.1,
                    0,
                    "case '\(testCase.name)': '\(last.0)' is in expected_order but scored 0"
                )
            }

            for id in testCase.zeroScore {
                let recipe = try XCTUnwrap(recipesByID[id], "case '\(testCase.name)': unknown id '\(id)'")
                XCTAssertEqual(
                    score(recipe, tokens: testCase.tokens),
                    0,
                    "case '\(testCase.name)': expected '\(id)' to score 0"
                )
            }
        }
    }

    private func score(_ recipe: FixtureRecipe, tokens: [String]) -> UInt32 {
        RecipeSearchSupport.relevanceScore(
            textTokens: tokens,
            document: RecipeSearchSupport.ScoringDocument(
                title: recipe.title,
                description: recipe.description,
                tags: recipe.tags,
                ingredients: recipe.ingredients,
                instructions: recipe.instructions,
                notes: recipe.notes
            )
        )
    }
}
