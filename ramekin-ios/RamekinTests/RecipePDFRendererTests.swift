import XCTest
import UIKit
@testable import Ramekin

final class RecipePDFRendererTests: XCTestCase {

    func testDefaultFilenameSlugifiesTitle() {
        XCTAssertEqual(
            RecipePDFRenderer.defaultFilename(for: "Chocolate Chip Cookies!"),
            "Chocolate-Chip-Cookies.pdf"
        )
    }

    func testDefaultFilenameTruncatesLongTitles() {
        let title = String(repeating: "a", count: 200)
        let name = RecipePDFRenderer.defaultFilename(for: title)
        XCTAssertTrue(name.hasSuffix(".pdf"))
        // Slug capped at 64 chars + extension.
        XCTAssertLessThanOrEqual(name.count, 64 + ".pdf".count)
    }

    func testDefaultFilenameFallsBackForEmptyTitle() {
        XCTAssertEqual(
            RecipePDFRenderer.defaultFilename(for: "   "),
            "recipe.pdf"
        )
    }

    func testRenderProducesNonEmptyPDF() throws {
        let recipe = Self.sampleRecipe()
        let result = try RecipePDFRenderer.render(
            recipe: recipe,
            coverPhoto: nil,
            filename: "test-render.pdf"
        )
        defer { try? FileManager.default.removeItem(at: result.url.deletingLastPathComponent()) }

        XCTAssertTrue(result.data.count > 200, "Rendered PDF should not be empty")
        // PDF files start with "%PDF" magic bytes.
        let prefix = result.data.prefix(4)
        XCTAssertEqual(prefix, Data("%PDF".utf8))
        XCTAssertTrue(FileManager.default.fileExists(atPath: result.url.path))
        XCTAssertEqual(result.url.lastPathComponent, "test-render.pdf")
    }

    private static func sampleRecipe() -> RecipeResponse {
        // Build the minimal RecipeResponse we need from the generated client.
        // The generated initializers accept all model fields; use defaults for
        // ones the PDF renderer doesn't read.
        let ingredient = Ingredient(
            item: "flour",
            measurements: [Measurement(amount: "2", unit: "cups")],
            note: nil,
            section: nil
        )
        return RecipeResponse(
            cookTime: "10 min",
            createdAt: Date(),
            description: "A simple test recipe.",
            difficulty: "easy",
            id: UUID(),
            ingredients: [ingredient],
            instructions: "Mix everything and bake.",
            notes: nil,
            nutritionalInfo: nil,
            photoIds: [],
            prepTime: "5 min",
            rating: nil,
            servings: "4",
            sourceName: "Example",
            sourceUrl: "https://example.com/recipe",
            tags: [],
            title: "Test Recipe",
            totalTime: "15 min",
            updatedAt: Date(),
            versionId: UUID(),
            versionSource: "manual"
        )
    }
}
