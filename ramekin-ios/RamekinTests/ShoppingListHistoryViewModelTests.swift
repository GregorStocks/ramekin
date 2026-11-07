import XCTest
@testable import Ramekin

@MainActor
final class ShoppingListHistoryViewModelTests: XCTestCase {
    private let entry = ShoppingListHistoryEntry(createdAt: Date(), id: UUID(), summary: "Added milk")

    func testUndoFailureSurvivesASuccessfulReload() async {
        let entry = entry
        let model = ShoppingListHistoryViewModel(
            loadHistory: { [entry] },
            undoEntry: { _ in throw URLError(.timedOut) }
        )
        await model.load()

        await model.undo(entry)
        XCTAssertNotNil(model.undoError)

        await model.load()

        XCTAssertNotNil(model.undoError, "A reload says nothing about the undo")
        XCTAssertNil(model.loadError)
        XCTAssertNil(model.undoingId)
    }

    func testSuccessfulUndoReloadsAndClearsEarlierUndoError() async {
        var failUndo = true
        var loads = 0
        let entry = entry
        let model = ShoppingListHistoryViewModel(
            loadHistory: {
                loads += 1
                return [entry]
            },
            undoEntry: { _ in if failUndo { throw URLError(.timedOut) } }
        )
        await model.load()
        await model.undo(entry)
        XCTAssertNotNil(model.undoError)

        failUndo = false
        await model.undo(entry)

        XCTAssertNil(model.undoError)
        XCTAssertEqual(loads, 2)
    }

    func testLoadFailureKeepsEntries() async {
        var failLoad = false
        let entry = entry
        let model = ShoppingListHistoryViewModel(
            loadHistory: {
                if failLoad { throw URLError(.timedOut) }
                return [entry]
            },
            undoEntry: { _ in }
        )
        await model.load()
        failLoad = true
        await model.load()

        XCTAssertNotNil(model.loadError)
        XCTAssertEqual(model.entries.map(\.id), [entry.id])
        XCTAssertFalse(model.isLoading)
    }
}
