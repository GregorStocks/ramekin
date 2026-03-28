import XCTest
@testable import Ramekin

final class RescrapeTests: XCTestCase {
    func testShouldShowRescrapeButtonRequiresNonEmptySourceUrl() {
        XCTAssertFalse(shouldShowRescrapeButton(sourceUrl: nil))
        XCTAssertFalse(shouldShowRescrapeButton(sourceUrl: ""))
        XCTAssertTrue(shouldShowRescrapeButton(sourceUrl: "https://example.com/recipe"))
    }

    func testShouldShowRescrapeButtonWithWhitespaceOnlyUrl() {
        // A whitespace-only URL is technically non-empty, but the server
        // would reject it. The button visibility mirrors the web app check
        // (non-nil and non-empty), so whitespace-only still shows the button.
        XCTAssertTrue(shouldShowRescrapeButton(sourceUrl: " "))
    }

    /// Mirrors the `if let sourceUrl = recipe.sourceUrl, !sourceUrl.isEmpty` check
    /// used in RecipeDetailView's toolbar menu.
    private func shouldShowRescrapeButton(sourceUrl: String?) -> Bool {
        guard let sourceUrl, !sourceUrl.isEmpty else {
            return false
        }
        _ = sourceUrl
        return true
    }
}
