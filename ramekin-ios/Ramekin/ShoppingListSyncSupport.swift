import CoreData
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
    /// it rather than the stale local copy being resent over it. That includes
    /// an edit made while the sync was in flight: updates send the whole row,
    /// so keeping it would overwrite the other device's change. A delete made
    /// while the sync was in flight still stands and is sent next round.
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
        if !success {
            if local.syncStatusEnum != .pendingDelete {
                local.markSynced(serverVersion: 0)
            }
        } else if !modifiedDuringSync {
            local.markSynced(serverVersion: Int32(version))
        } else {
            local.serverVersion = Int32(version)
            if local.syncStatusEnum == .pendingCreate {
                local.syncStatusEnum = .pendingUpdate
            }
        }
        return .kept
    }

    /// A full resync returns every live item, so a synced item it omits is
    /// gone from the server without a tombstone (e.g. created after a
    /// restored backup was taken). So is a pending delete it omits: the
    /// server can't acknowledge deleting a row it never had, and retrying
    /// would never settle it. Pending creates and updates stay: their own
    /// sync round settles them.
    static func dropItemsMissingFromFullResync(
        _ response: SyncResponse,
        accountKey: String,
        in context: NSManagedObjectContext
    ) {
        let live = Set(response.serverChanges.map(\.id))
        let settled = (try? context.fetch(ShoppingItem.fetchSyncedOrPendingDelete(accountKey: accountKey))) ?? []
        for item in settled {
            if let id = item.id, !live.contains(id) {
                context.delete(item)
            }
        }
    }
}
