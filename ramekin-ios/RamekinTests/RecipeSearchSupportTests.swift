import XCTest
@testable import Ramekin

/// Unit tests for the query parser and literal substring matching. End-to-end behavior is
/// pinned by SearchMatchFilterSharedVectorTests; these cover the parser and
/// matcher edges directly, mirroring the server's parse_query tests.
final class RecipeSearchSupportTests: XCTestCase {
    func testParsePlainText() {
        let parsed = RecipeSearchSupport.parse("chicken soup")
        XCTAssertEqual(parsed.textTokens, ["chicken", "soup"])
        XCTAssertTrue(parsed.tags.isEmpty)
        XCTAssertFalse(parsed.requiresServer)
    }

    func testParseQuotedPhraseIsOneToken() {
        let parsed = RecipeSearchSupport.parse("\"green beans\" tag:side")
        XCTAssertEqual(parsed.textTokens, ["green beans"])
        XCTAssertEqual(parsed.tags, ["side"])
    }

    func testParseMixedFilters() {
        let parsed = RecipeSearchSupport.parse("chicken tag:dinner source:NYTimes has:photos")
        XCTAssertEqual(parsed.textTokens, ["chicken"])
        XCTAssertEqual(parsed.tags, ["dinner"])
        XCTAssertEqual(parsed.source, "NYTimes")
        XCTAssertEqual(parsed.photoFilter, .hasPhotos)
        XCTAssertTrue(parsed.requiresServer)
    }

    func testParseNoPhotosToken() {
        XCTAssertEqual(RecipeSearchSupport.parse("no:photos").photoFilter, .noPhotos)
        XCTAssertEqual(RecipeSearchSupport.parse("no:photo").photoFilter, .noPhotos)
        XCTAssertEqual(RecipeSearchSupport.parse("bread").photoFilter, .any)
    }

    func testParseDateForms() {
        let after = RecipeSearchSupport.parse("created:>2024-01-15")
        XCTAssertEqual(after.createdAfter, "2024-01-15")
        XCTAssertNil(after.createdBefore)

        let before = RecipeSearchSupport.parse("created:<2024-12-31")
        XCTAssertNil(before.createdAfter)
        XCTAssertEqual(before.createdBefore, "2024-12-31")

        let range = RecipeSearchSupport.parse("created:2024-01-01..2024-06-30")
        XCTAssertEqual(range.createdAfter, "2024-01-01")
        XCTAssertEqual(range.createdBefore, "2024-06-30")

        let exact = RecipeSearchSupport.parse("created:2024-03-15")
        XCTAssertEqual(exact.createdAfter, "2024-03-15")
        XCTAssertEqual(exact.createdBefore, "2024-03-15")
    }

    func testParseUnparseableFilterValuesAreNoOps() {
        // Consumed by their filter branch without becoming text terms,
        // exactly like the server.
        let parsed = RecipeSearchSupport.parse("created:notadate photo_size:abc photo_dim:12")
        XCTAssertTrue(parsed.textTokens.isEmpty)
        XCTAssertNil(parsed.createdAfter)
        XCTAssertNil(parsed.createdBefore)
        XCTAssertNil(parsed.photoSize)
        XCTAssertNil(parsed.photoDim)
        XCTAssertFalse(parsed.requiresServer)
    }

    func testParseUnknownPrefixIsText() {
        XCTAssertEqual(RecipeSearchSupport.parse("foo:bar").textTokens, ["foo:bar"])
    }

    func testParseNumericThresholds() {
        let parsed = RecipeSearchSupport.parse("photo_size:<100000 photo_dim:>500")
        XCTAssertEqual(parsed.photoSize, NumericThreshold(comparison: .lessThan, value: 100_000))
        XCTAssertEqual(parsed.photoDim, NumericThreshold(comparison: .greaterThan, value: 500))
        XCTAssertTrue(parsed.requiresServer)
    }

    func testNormalizedMetacharactersMatchLiterally() {
        for (query, literal) in [("％", "%"), ("﹪", "%"), ("＿", "_"),
                                 ("∖", "\\"), ("﹨", "\\"), ("＼", "\\")] {
            let needle = RecipeSearchSupport.normalizedScalars("a" + query + "b")
            XCTAssertTrue(RecipeSearchSupport.scalarsContain(scalars("a" + literal + "b"), needle))
            XCTAssertFalse(RecipeSearchSupport.scalarsContain(scalars("axb ab"), needle))
        }
    }

    private func scalars(_ text: String) -> [UInt32] {
        Array(text.unicodeScalars.map(\.value))
    }
}
