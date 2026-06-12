import XCTest
@testable import Ramekin

final class RecipeExportSupportTests: XCTestCase {

    func testParseFilenameFromQuotedAttribute() {
        let header = "attachment; filename=\"Chocolate Cake.paprikarecipe\""
        XCTAssertEqual(
            RecipeExportSupport.parseContentDispositionFilename(header),
            "Chocolate Cake.paprikarecipe"
        )
    }

    func testParseFilenameFromBareAttribute() {
        let header = "attachment; filename=recipes-20260611.paprikarecipes"
        XCTAssertEqual(
            RecipeExportSupport.parseContentDispositionFilename(header),
            "recipes-20260611.paprikarecipes"
        )
    }

    func testParseFilenameStarPrefersRFC5987WhenPresent() {
        let header = "attachment; filename=\"fallback.bin\"; filename*=UTF-8''Ch%C3%A9z%20Caf%C3%A9.paprikarecipe"
        XCTAssertEqual(
            RecipeExportSupport.parseContentDispositionFilename(header),
            "Chéz Café.paprikarecipe"
        )
    }

    func testParseFilenameReturnsNilForMissingHeader() {
        XCTAssertNil(RecipeExportSupport.parseContentDispositionFilename(nil))
        XCTAssertNil(RecipeExportSupport.parseContentDispositionFilename(""))
    }

    func testParseFilenameStripsPathComponents() {
        let header = "attachment; filename=\"../../etc/passwd\""
        // We only keep the last path component and reject path separators.
        XCTAssertEqual(
            RecipeExportSupport.parseContentDispositionFilename(header),
            "passwd"
        )
    }

    func testParseFilenameRejectsControlCharacters() {
        let header = "attachment; filename=\"bad\u{07}name.bin\""
        XCTAssertEqual(
            RecipeExportSupport.parseContentDispositionFilename(header),
            "badname.bin"
        )
    }

    func testSuggestedFilenameUsesFallbackWhenHeaderMissing() {
        let fallback = "fallback.paprikarecipe"
        XCTAssertEqual(
            RecipeExportSupport.suggestedFilename(from: nil, fallback: fallback),
            fallback
        )
    }

    func testSuggestedFilenamePrefersServerHeader() {
        let header = "attachment; filename=\"server.paprikarecipe\""
        XCTAssertEqual(
            RecipeExportSupport.suggestedFilename(from: header, fallback: "fallback.bin"),
            "server.paprikarecipe"
        )
    }

    func testFallbackFilenamesIncludeExpectedExtensions() {
        XCTAssertTrue(
            RecipeExportSupport.fallbackSingleRecipeFilename().hasSuffix(".paprikarecipe"),
            "single-recipe fallback should use .paprikarecipe extension"
        )
        XCTAssertTrue(
            RecipeExportSupport.fallbackAllRecipesFilename().hasSuffix(".paprikarecipes"),
            "all-recipes fallback should use .paprikarecipes extension"
        )
    }

    func testWriteToTempFileCreatesFileAndReturnsURL() throws {
        let data = Data("hello".utf8)
        let url = try RecipeExportSupport.writeToTempFile(data: data, filename: "x.paprikarecipe")
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }

        XCTAssertEqual(url.lastPathComponent, "x.paprikarecipe")
        XCTAssertTrue(FileManager.default.fileExists(atPath: url.path))
        let roundTrip = try Data(contentsOf: url)
        XCTAssertEqual(roundTrip, data)
    }

    func testWriteToTempFileSanitizesPathSeparators() throws {
        let data = Data("hello".utf8)
        let url = try RecipeExportSupport.writeToTempFile(
            data: data,
            filename: "../escape.paprikarecipe"
        )
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }

        // We should not have escaped the ramekin-exports subdirectory.
        XCTAssertEqual(url.lastPathComponent, "escape.paprikarecipe")
        XCTAssertTrue(url.path.contains("ramekin-exports"))
    }
}
