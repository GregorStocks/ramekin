import Foundation

enum ShoppingListSyncSupport {
    enum Reconciled: Equatable {
        case kept
        /// The server no longer has the item (another device deleted it).
        case deletedOnServer
    }

    /// Apply the server's answer to a pending create or update.
    ///
    /// A rejected update means another device changed the item first. The
    /// server's copy wins: the item is marked synced at version 0, so the
    /// server's row, which comes back in the same response's changes, replaces
    /// it rather than the stale local copy being resent over it. An edit made
    /// while the sync was in flight is newer than both, so it stays pending
    /// against the server's version.
    static func reconcileSyncedItem(
        _ local: ShoppingItem,
        version: Int,
        success: Bool,
        syncStartedAt: Date
    ) -> Reconciled {
        let modifiedDuringSync = local.updatedAt ?? Date.distantPast > syncStartedAt

        if !success && version == 0 {
            return .deletedOnServer
        }
        if success && !modifiedDuringSync {
            local.markSynced(serverVersion: Int32(version))
        } else if !success && !modifiedDuringSync {
            local.markSynced(serverVersion: 0)
        } else {
            local.serverVersion = Int32(version)
            if local.syncStatusEnum == .pendingCreate {
                local.syncStatusEnum = .pendingUpdate
            }
        }
        return .kept
    }
}
