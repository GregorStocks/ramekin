import XCTest
@testable import Ramekin

final class ShareExtensionTests: XCTestCase {

    // MARK: - URL Validation Tests

    func testValidRecipeURLs() {
        let validURLs = [
            "https://www.allrecipes.com/recipe/12345/chocolate-cake",
            "https://cooking.nytimes.com/recipes/1234-pasta",
            "https://www.seriouseats.com/best-chocolate-chip-cookies",
            "https://www.bonappetit.com/recipe/chocolate-cake",
            "https://example.com/my-recipe"
        ]

        for urlString in validURLs {
            let url = URL(string: urlString)
            XCTAssertNotNil(url, "Should parse valid URL: \(urlString)")
            XCTAssertTrue(url!.scheme == "https" || url!.scheme == "http",
                         "URL should have http(s) scheme: \(urlString)")
        }
    }

    func testURLWithQueryParameters() {
        let urlString = "https://example.com/recipe?id=123&source=share"
        let url = URL(string: urlString)

        XCTAssertNotNil(url)
        XCTAssertEqual(url?.host, "example.com")
        XCTAssertEqual(url?.path, "/recipe")
        XCTAssertNotNil(url?.query)
    }

    func testURLWithFragment() {
        let urlString = "https://example.com/recipe#ingredients"
        let url = URL(string: urlString)

        XCTAssertNotNil(url)
        XCTAssertEqual(url?.fragment, "ingredients")
    }

    func testInternationalURLs() {
        // URLs with international characters should be handled
        let urlString = "https://example.com/recette/gâteau"
        let url = URL(string: urlString.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? urlString)

        XCTAssertNotNil(url)
    }

    // MARK: - Share Status Tests

    func testShareStatusTransitions() {
        // This tests the conceptual state machine of share statuses
        enum ShareStatus: Equatable {
            case ready
            case sending
            case success
            case error
            case notLoggedIn
        }

        // Valid transitions
        var status: ShareStatus = .ready

        // ready -> sending (user initiates share)
        status = .sending
        XCTAssertEqual(status, .sending)

        // sending -> success (API call succeeds)
        status = .success
        XCTAssertEqual(status, .success)

        // Reset and test error path
        status = .ready
        status = .sending
        status = .error
        XCTAssertEqual(status, .error)

        // error -> sending (user retries)
        status = .sending
        XCTAssertEqual(status, .sending)

        // Test not logged in path
        status = .ready
        status = .notLoggedIn
        XCTAssertEqual(status, .notLoggedIn)
    }

    // MARK: - URL Extraction Simulation

    func testExtractURLFromText() {
        // Simulates extracting URL from plain text (as might come from some share sources)
        let texts = [
            "https://example.com/recipe",
            "Check out this recipe: https://example.com/recipe",
            "   https://example.com/recipe   "
        ]

        for text in texts {
            // Simple URL extraction logic
            let pattern = "https?://[^\\s]+"
            if let regex = try? NSRegularExpression(pattern: pattern),
               let match = regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)),
               let range = Range(match.range, in: text) {
                let urlString = String(text[range])
                let url = URL(string: urlString)
                XCTAssertNotNil(url, "Should extract URL from: \(text)")
            } else {
                XCTFail("Should find URL in: \(text)")
            }
        }
    }
}
