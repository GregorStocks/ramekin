import XCTest
@testable import Ramekin

final class TextRecipeDraftTests: XCTestCase {
    func testSharedDraftRoundTripsPreserveContent() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "text-recipe-draft", withExtension: "json"))
        let data = try Data(contentsOf: url)
        let drafts = try JSONDecoder().decode([RecipeContent].self, from: data)
        let expected = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [NSDictionary])
        for (index, draft) in drafts.enumerated() {
            let request = RecipeFormData(content: draft).makeCreateRequest()
            let result = try JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? NSDictionary
            XCTAssertEqual(result, expected[index])
        }
    }
}
