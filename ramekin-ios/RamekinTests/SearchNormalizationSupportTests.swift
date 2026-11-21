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

    /// The corpus is normalized through the byte-level path, so it has to
    /// agree with the contract's per-scalar definition everywhere.
    func testNormalizedUTF8MatchesNormalizeForSearchForEveryScalar() {
        var chunk = String.UnicodeScalarView()
        var chunkLength = 0
        func flush() {
            let text = String(chunk)
            let expected = Array(SearchNormalizationSupport.normalizeForSearch(text).utf8)
            if SearchNormalizationSupport.normalizedUTF8(text) != expected {
                XCTFail("normalizedUTF8 diverges within \(text.unicodeScalars.map { String($0.value, radix: 16) })")
            }
            chunk = String.UnicodeScalarView()
            chunkLength = 0
        }
        for value in UInt32(0)...0x10FFFF {
            guard let scalar = Unicode.Scalar(value) else { continue }
            chunk.append(scalar)
            chunkLength += 1
            if chunkLength == 64 {
                flush()
            }
        }
        flush()
    }

    func testNormalizedUTF8HandlesMixedText() {
        for text in ["", "plain", "Crème Brûlée", "1½ cups ﬁnely chopped JALAPEÑO 🌶️ at 350°F",
                     "Cre\u{301}me", "ΣΟΥΠΑ", "ＡＢＣ", "Mom’s Sweet–and–Sour 𝒮auce"] {
            XCTAssertEqual(
                SearchNormalizationSupport.normalizedUTF8(text),
                Array(SearchNormalizationSupport.normalizeForSearch(text).utf8),
                text
            )
        }
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
