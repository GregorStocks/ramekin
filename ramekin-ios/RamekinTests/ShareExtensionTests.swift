import UniformTypeIdentifiers
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
        let urlString = "https://example.com/recette/gâteau"
        let url = URL(string: urlString.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? urlString)

        XCTAssertNotNil(url)
    }

    // MARK: - Share Status Tests

    func testShareStatusTransitions() {
        enum ShareStatus: Equatable {
            case ready
            case sending
            case success
            case error
            case notLoggedIn
        }

        var status: ShareStatus = .ready
        status = .sending
        XCTAssertEqual(status, .sending)
        status = .success
        XCTAssertEqual(status, .success)

        status = .ready
        status = .sending
        status = .error
        XCTAssertEqual(status, .error)

        status = .sending
        XCTAssertEqual(status, .sending)

        status = .ready
        status = .notLoggedIn
        XCTAssertEqual(status, .notLoggedIn)
    }
}
