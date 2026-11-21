import XCTest
@testable import Ramekin

@MainActor
final class RecipePickerViewModelTests: XCTestCase {
    func testSlowOlderResponseCannotOverwriteNewerOne() async {
        let soup = RecipeListTestSupport.makeRecipe(title: "Soup")
        let cake = RecipeListTestSupport.makeRecipe(title: "Cake")
        let browseStarted = RequestGate()
        let releaseBrowse = RequestGate()
        let model = RecipePickerViewModel(listRecipes: { query in
            if query == nil {
                await browseStarted.open()
                await releaseBrowse.wait()
                return [soup]
            }
            return [cake]
        })

        let initial = Task { await model.load(searchText: "") }
        await browseStarted.wait()
        await model.load(searchText: "cake")
        XCTAssertEqual(model.recipes.map(\.id), [cake.id])
        XCTAssertFalse(model.isLoading)

        releaseBrowse.open()
        await initial.value

        XCTAssertEqual(model.recipes.map(\.id), [cake.id])
        XCTAssertFalse(model.isLoading)
    }

    func testStartShowsTheCacheThenWhatSyncBroughtIn() async {
        let cached = RecipeListTestSupport.makeRecipe(title: "Cached Cake")
        let synced = RecipeListTestSupport.makeRecipe(title: "Synced Cake")
        var cache = [cached]
        var shownBeforeSync: [UUID] = []
        var model: RecipePickerViewModel?
        model = RecipePickerViewModel(
            listRecipes: { query in query == "cake" ? cache : [] },
            freshen: {
                shownBeforeSync = model?.recipes.map(\.id) ?? []
                cache.append(synced)
                // The user kept typing while the sync ran.
                await model?.load(searchText: "cake")
            }
        )

        await model?.start(searchText: "cake")

        XCTAssertEqual(shownBeforeSync, [cached.id])
        XCTAssertEqual(model?.recipes.map(\.id), [cached.id, synced.id])
        XCTAssertEqual(model?.isLoading, false)
    }

    func testOlderLoadFinishingDoesNotClearNewerSpinner() async {
        let firstStarted = RequestGate()
        let releaseFirst = RequestGate()
        let secondStarted = RequestGate()
        let releaseSecond = RequestGate()
        let model = RecipePickerViewModel(listRecipes: { query in
            if query == "a" {
                await firstStarted.open()
                await releaseFirst.wait()
            } else {
                await secondStarted.open()
                await releaseSecond.wait()
            }
            return []
        })

        let first = Task { await model.load(searchText: "a") }
        await firstStarted.wait()
        let second = Task { await model.load(searchText: "b") }
        await secondStarted.wait()

        releaseFirst.open()
        await first.value
        XCTAssertTrue(model.isLoading, "The newer load is still running")

        releaseSecond.open()
        await second.value
        XCTAssertFalse(model.isLoading)
    }

    func testCancelledLoadClearsSpinnerWithoutError() async {
        let model = RecipePickerViewModel(listRecipes: { _ in throw URLError(.cancelled) })

        await model.load(searchText: "")

        XCTAssertFalse(model.isLoading)
        XCTAssertNil(model.error)
    }

    func testFailureShowsError() async {
        let model = RecipePickerViewModel(listRecipes: { _ in throw URLError(.timedOut) })

        await model.load(searchText: "")

        XCTAssertFalse(model.isLoading)
        XCTAssertNotNil(model.error)
    }
}
