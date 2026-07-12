import XCTest
@testable import Ramekin

/// Mirrors the Rust unit tests in ramekin-core/src/search.rs; both sides
/// consume the same shared-test-vectors/search-normalization.json contract.
final class SearchNormalizationSupportTests: XCTestCase {
    func testContractVersionIsPositive() {
        XCTAssertGreaterThanOrEqual(SearchNormalizationSupport.contractVersion, 1)
    }

    func testNormalizeStripsAccentsAndCase() {
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("Crème Brûlée"), "creme brulee")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("JALAPEÑO"), "jalapeno")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("plain"), "plain")
    }

    func testNormalizeExpandsLigatureLetters() {
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("Œufs"), "oeufs")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("Æbleskiver"), "aebleskiver")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("Spätzle mit Soße"), "spatzle mit sosse")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("Smørrebrød"), "smorrebrod")
    }

    func testNormalizeExpandsPresentationFormsAndPunctuation() {
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("ﬁnely chopped"), "finely chopped")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("1½ cups"), "11/2 cups")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("Mom’s Apple Cake"), "mom's apple cake")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("Sweet–and–Sour"), "sweet-and-sour")
    }

    func testNormalizeDeletesCombiningMarks() {
        // Decomposed "é" (e + U+0301): the dictionary deletes the bare
        // combining mark, exactly like the database.
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("Cre\u{301}me"), "creme")
    }

    func testNormalizeFoldsFullwidthForms() {
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("ＡＢＣ"), "abc")
    }

    func testNormalizeLowercasesPerCodepointWithoutContext() {
        // The database's lower() has no final-sigma special case: Σ always
        // lowercases to σ, and ς stays ς. Swift's lowercased() would produce
        // "ας" for "ΑΣ"; the contract must win.
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("ΣΟΥΠΑ"), "σουπα")
        XCTAssertEqual(SearchNormalizationSupport.normalizeForSearch("σουπες"), "σουπες")
    }

    func testCitextFoldKeepsAccents() {
        XCTAssertEqual(SearchNormalizationSupport.citextFold("CRÈME"), "crème")
        XCTAssertEqual(SearchNormalizationSupport.citextFold("Dinner"), "dinner")
        // No unaccent step: the accented and plain forms stay distinct.
        XCTAssertNotEqual(
            SearchNormalizationSupport.citextFold("Crème"),
            SearchNormalizationSupport.citextFold("Creme")
        )
    }
}
