import XCTest
@testable import Ramekin

final class IngredientRowViewSupportTests: XCTestCase {
    func testNoteFieldHiddenForFilledIngredientWithoutNoteUntilExplicitlyShown() {
        XCTAssertFalse(
            IngredientRowViewSupport.shouldShowNoteField(
                item: "flour",
                note: "",
                isNoteVisible: false
            )
        )
        XCTAssertTrue(
            IngredientRowViewSupport.shouldShowAddNoteButton(
                item: "flour",
                note: "",
                isNoteVisible: false
            )
        )
    }

    func testNoteFieldVisibleForEmptyIngredientWithoutPersistingPlaceholderText() {
        XCTAssertTrue(
            IngredientRowViewSupport.shouldShowNoteField(
                item: "",
                note: "",
                isNoteVisible: false
            )
        )

        let ingredient = EditableIngredient.empty().toIngredient()
        XCTAssertNil(ingredient.note)
    }

    func testNoteFieldStaysVisibleAfterUserRequestsItWithoutChangingSavedIngredient() {
        XCTAssertTrue(
            IngredientRowViewSupport.shouldShowNoteField(
                item: "flour",
                note: "",
                isNoteVisible: true
            )
        )
        XCTAssertFalse(
            IngredientRowViewSupport.shouldShowAddNoteButton(
                item: "flour",
                note: "",
                isNoteVisible: true
            )
        )

        let ingredient = EditableIngredient(
            item: "flour",
            measurements: [EditableMeasurement(amount: "1", unit: "cup")],
            note: "",
            section: ""
        ).toIngredient()
        XCTAssertNil(ingredient.note)
    }

    func testExistingNoteStillShowsFieldAndRoundTrips() {
        XCTAssertTrue(
            IngredientRowViewSupport.shouldShowNoteField(
                item: "flour",
                note: "sifted",
                isNoteVisible: false
            )
        )
        XCTAssertFalse(
            IngredientRowViewSupport.shouldShowAddNoteButton(
                item: "flour",
                note: "sifted",
                isNoteVisible: false
            )
        )

        let ingredient = EditableIngredient(
            item: "flour",
            measurements: [EditableMeasurement(amount: "1", unit: "cup")],
            note: "sifted",
            section: ""
        ).toIngredient()
        XCTAssertEqual(ingredient.note, "sifted")
    }
}
