import XCTest
@testable import Ramekin

@MainActor
final class TagManagementViewModelTests: XCTestCase {
    private let dinner = TagManagementViewModelTests.makeTag("Dinner")
    private let lunch = TagManagementViewModelTests.makeTag("Lunch")
    private var accountKey: String? = "tag-tests-\(UUID().uuidString)"

    func testRenameFinishingDoesNotCloseAnotherTagsEditor() async {
        let renameStarted = RequestGate()
        let releaseRename = RequestGate()
        let model = makeModel(renameTag: { _, name in
            await renameStarted.open()
            await releaseRename.wait()
            return name
        })
        await model.loadTags()

        model.startEditing(dinner)
        model.editName = "Supper"
        let save = Task { await model.saveEdit() }
        await renameStarted.wait()

        model.startEditing(lunch)
        model.editName = "Brunch"
        releaseRename.open()
        await save.value

        XCTAssertEqual(model.tags.map(\.name).sorted(), ["Lunch", "Supper"])
        XCTAssertEqual(model.editingTagId, lunch.id)
        XCTAssertEqual(model.editName, "Brunch")
        XCTAssertNil(model.savingTagId)
    }

    func testRenameFailureForAnotherTagDoesNotShowOnOpenEditor() async {
        let renameStarted = RequestGate()
        let releaseRename = RequestGate()
        let model = makeModel(renameTag: { _, _ in
            await renameStarted.open()
            await releaseRename.wait()
            throw URLError(.timedOut)
        })
        await model.loadTags()

        model.startEditing(dinner)
        model.editName = "Supper"
        let save = Task { await model.saveEdit() }
        await renameStarted.wait()
        model.startEditing(lunch)
        releaseRename.open()
        await save.value

        XCTAssertNil(model.editError)
    }

    func testRepeatedSubmitSendsOneRename() async {
        var renameCalls = 0
        let renameStarted = RequestGate()
        let releaseRename = RequestGate()
        let model = makeModel(renameTag: { _, name in
            renameCalls += 1
            await renameStarted.open()
            await releaseRename.wait()
            return name
        })
        await model.loadTags()
        model.startEditing(dinner)
        model.editName = "Supper"

        let first = Task { await model.saveEdit() }
        await renameStarted.wait()
        XCTAssertTrue(model.isSavingEdit)
        await model.saveEdit()
        releaseRename.open()
        await first.value

        XCTAssertEqual(renameCalls, 1)
        XCTAssertNil(model.editingTagId)
    }

    func testRenameAfterAccountChangeClearsSavingState() async {
        let model = makeModel(renameTag: { [unowned self] _, name in
            accountKey = "someone-else"
            return name
        })
        await model.loadTags()
        model.startEditing(dinner)
        model.editName = "Supper"

        await model.saveEdit()

        XCTAssertNil(model.savingTagId)
        XCTAssertEqual(model.tags.map(\.name).sorted(), ["Dinner", "Lunch"])
    }

    func testLoadAfterAccountChangeClearsSpinner() async {
        let model = makeModel(listTags: { [unowned self] in
            accountKey = "someone-else"
            return [dinner]
        })

        await model.loadTags()

        XCTAssertFalse(model.isLoading)
        XCTAssertTrue(model.tags.isEmpty)
    }

    func testDeleteSuccessKeepsLoadErrorAndTracksProgress() async {
        var failLoad = false
        let deleteStarted = RequestGate()
        let releaseDelete = RequestGate()
        let model = makeModel(
            listTags: { [unowned self] in
                if failLoad { throw URLError(.timedOut) }
                return [dinner, lunch]
            },
            deleteTag: { _ in
                await deleteStarted.open()
                await releaseDelete.wait()
            }
        )
        await model.loadTags()
        failLoad = true
        await model.loadTags()
        XCTAssertNotNil(model.loadError)

        let delete = Task { await model.delete(dinner) }
        await deleteStarted.wait()
        XCTAssertTrue(model.isDeleting(dinner))
        releaseDelete.open()
        await delete.value

        XCTAssertFalse(model.isDeleting(dinner))
        XCTAssertEqual(model.tags.map(\.name), ["Lunch"])
        XCTAssertNotNil(model.loadError, "A delete says nothing about whether the list loaded")
    }

    func testDeleteFailureReportsSeparately() async {
        let model = makeModel(deleteTag: { _ in throw URLError(.timedOut) })
        await model.loadTags()

        await model.delete(dinner)

        XCTAssertNotNil(model.deleteError)
        XCTAssertNil(model.loadError)
        XCTAssertFalse(model.isDeleting(dinner))
    }

    private func makeModel(
        listTags: (() async throws -> [TagItem])? = nil,
        renameTag: @escaping (UUID, String) async throws -> String = { _, name in name },
        deleteTag: @escaping (UUID) async throws -> Void = { _ in }
    ) -> TagManagementViewModel {
        let tags = [dinner, lunch]
        return TagManagementViewModel(client: TagManagementClient(
            currentAccountKey: { [unowned self] in accountKey },
            listTags: listTags ?? { tags },
            renameTag: renameTag,
            deleteTag: deleteTag
        ))
    }

    private static func makeTag(_ name: String) -> TagItem {
        TagItem(createdAt: Date(), id: UUID(), name: name, namespace: nil, recipeCount: 1, value: name)
    }
}
