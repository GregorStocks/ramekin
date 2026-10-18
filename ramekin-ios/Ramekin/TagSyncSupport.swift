import Foundation

/// Keeping the cached available tags (the filter chips and the recipe
/// form's suggestions) up to date from `GET /api/sync/tags`.
enum TagSyncSupport {
    /// Fetch the tag changes since the cached cursor, apply them to the
    /// account's cached tags, and return the result sorted by name.
    @discardableResult
    static func refresh(
        accountKey: String,
        userDefaults: UserDefaults = .standard,
        fetch: (_ cursor: Int64?) async throws -> SyncTagsResponse
    ) async throws -> [SyncTag] {
        let response = try await fetch(TagFilterCache.tagsSyncCursor(accountKey: accountKey, userDefaults: userDefaults))
        let cached = TagFilterCache.loadAvailableTags(accountKey: accountKey, userDefaults: userDefaults)
        let tags = apply(response, to: cached)
        TagFilterCache.saveAvailableTags(tags, accountKey: accountKey, userDefaults: userDefaults)
        TagFilterCache.setTagsSyncCursor(response.cursor, accountKey: accountKey, userDefaults: userDefaults)
        return tags
    }

    /// Tags carry no change stamp (they're never edited offline), so every
    /// server row is current.
    static func apply(_ response: SyncTagsResponse, to cached: [SyncTag]) -> [SyncTag] {
        let byId = Dictionary(cached.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        let changes = Dictionary(response.changes.map { ($0.id, $0) }, uniquingKeysWith: { _, last in last })
        let actions = SyncApplySupport.plan(
            cached: byId.mapValues { _ in SyncApplySupport.CachedState(changeSeq: 0, pending: nil) },
            changes: response.changes.map { (id: $0.id, changeSeq: 0) },
            deleted: response.deleted,
            fullResync: response.fullResync
        )
        return actions
            .compactMap { id, action -> SyncTag? in
                switch action {
                case .drop: return nil
                case .keep: return byId[id]
                case .take: return changes[id]
                }
            }
            .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }
}
