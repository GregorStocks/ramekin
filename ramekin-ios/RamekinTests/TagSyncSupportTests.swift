import XCTest
@testable import Ramekin

final class TagSyncSupportTests: XCTestCase {
    func testDeltaAddsRenamesAndDeletesTags() {
        let dinner = SyncTag(changeSeq: 1, id: UUID(), name: "Dinner")
        let quick = SyncTag(changeSeq: 1, id: UUID(), name: "Quick")
        let gone = SyncTag(changeSeq: 1, id: UUID(), name: "Gone")
        let added = SyncTag(changeSeq: 1, id: UUID(), name: "Added")

        let tags = TagSyncSupport.apply(
            SyncTagsResponse(
                changes: [SyncTag(changeSeq: 5, id: quick.id, name: "Fast"), added],
                cursor: 9,
                deleted: [gone.id],
                fullResync: false
            ),
            to: [dinner, quick, gone]
        )

        XCTAssertEqual(tags.map(\.name), ["Added", "Dinner", "Fast"])
    }

    func testFullResyncDropsTagsItOmits() {
        let kept = SyncTag(changeSeq: 1, id: UUID(), name: "Kept")
        let lost = SyncTag(changeSeq: 1, id: UUID(), name: "Lost")

        let tags = TagSyncSupport.apply(
            SyncTagsResponse(changes: [kept], cursor: 9, deleted: [], fullResync: true),
            to: [kept, lost]
        )

        XCTAssertEqual(tags, [kept])
    }

    func testRefreshResumesFromTheStoredCursor() async throws {
        let suiteName = "TagSyncSupportTests.\(name)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let accountKey = AccountScope.key(serverURL: "https://one.test", username: "chef")
        var cursors: [Int64?] = []
        let dinner = SyncTag(changeSeq: 1, id: UUID(), name: "Dinner")

        for _ in 0..<2 {
            try await TagSyncSupport.refresh(accountKey: accountKey, userDefaults: defaults) { cursor in
                cursors.append(cursor)
                return SyncTagsResponse(changes: [dinner], cursor: 42, deleted: [], fullResync: cursor == nil)
            }
        }

        XCTAssertEqual(cursors, [nil, 42])
        XCTAssertEqual(TagFilterCache.loadAvailableTags(accountKey: accountKey, userDefaults: defaults), [dinner])
    }
}
