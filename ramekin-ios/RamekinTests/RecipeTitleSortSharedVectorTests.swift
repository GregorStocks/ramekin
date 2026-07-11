import XCTest
@testable import Ramekin

final class RecipeTitleSortSharedVectorTests: XCTestCase {
    private struct Vectors: Decodable {
        let recipes: [Recipe]
        let ascending: [UUID]
        let descending: [UUID]
    }

    private struct Recipe: Decodable {
        let id: UUID
        let title: String
    }

    func testRecipeTitleSortMatchesSharedVectors() throws {
        let vectors = try loadVectors()

        XCTAssertEqual(sortedIDs(vectors.recipes, descending: false), vectors.ascending)
        XCTAssertEqual(sortedIDs(vectors.recipes, descending: true), vectors.descending)
    }

    private func sortedIDs(_ recipes: [Recipe], descending: Bool) -> [UUID] {
        recipes.sorted { lhs, rhs in
            RecipeTitleSortSupport.areInIncreasingOrder(
                lhsTitle: lhs.title,
                lhsID: lhs.id,
                rhsTitle: rhs.title,
                rhsID: rhs.id,
                descending: descending
            )
        }.map(\.id)
    }

    private func loadVectors() throws -> Vectors {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "recipe-title-sort", withExtension: "json")
        )
        return try JSONDecoder().decode(Vectors.self, from: Data(contentsOf: url))
    }
}
