import Foundation

/// Applying a sync response (doc/sync-design.md) to a cached table, pinned
/// with web by shared-test-vectors/sync-apply.json.
enum SyncApplySupport {
    /// An unsent local write waiting in a row's outbox.
    enum PendingWrite: String {
        case create
        case update
        case delete
    }

    /// What a cached row needs to know to be planned.
    struct CachedState {
        let changeSeq: Int64
        let pending: PendingWrite?
    }

    enum Action: Equatable {
        /// Write the server's row over every field that isn't dirty
        /// (inserting it if the cache lacks it).
        case take
        /// Leave the cached row as it is.
        case keep
        /// Delete the cached row.
        case drop
    }

    /// The action for every row the cache or the response holds.
    /// Tombstones drop rows; a server row older than the cached one is
    /// skipped, except on a full resync; a full resync drops cached rows it
    /// omits, except pending creates and updates, which their own ops settle.
    static func plan<ID: Hashable>(
        cached: [ID: CachedState],
        changes: [(id: ID, changeSeq: Int64)],
        deleted: [ID],
        fullResync: Bool
    ) -> [ID: Action] {
        let deleted = Set(deleted)
        var actions: [ID: Action] = [:]
        for (id, state) in cached {
            if deleted.contains(id) {
                actions[id] = .drop
            } else if fullResync, state.pending != .create, state.pending != .update {
                actions[id] = .drop
            } else {
                actions[id] = .keep
            }
        }
        for change in changes where !deleted.contains(change.id) {
            // A full resync's stamps may come from another database, so they
            // can't be compared with the cached ones.
            if !fullResync, let state = cached[change.id], state.changeSeq > change.changeSeq {
                actions[change.id] = .keep
            } else {
                actions[change.id] = .take
            }
        }
        return actions
    }
}

/// The fields of a row with unsent local edits, stored as a comma-separated
/// list of API field names. A pending update stored before field tracking
/// has none recorded, and counts every field as dirty.
enum DirtyFields {
    static func decode(_ stored: String?, allFields: [String]) -> Set<String> {
        guard let stored else { return Set(allFields) }
        return Set(stored.split(separator: ",").map(String.init))
    }

    static func encode(_ fields: Set<String>) -> String {
        fields.sorted().joined(separator: ",")
    }
}
