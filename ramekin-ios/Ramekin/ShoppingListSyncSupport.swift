import Foundation

enum ShoppingListSyncSupport {
    static func reconcileSyncedItem(
        _ local: ShoppingItem,
        version: Int,
        success: Bool,
        syncStartedAt: Date
    ) {
        let modifiedDuringSync = local.updatedAt ?? Date.distantPast > syncStartedAt

        if success && !modifiedDuringSync {
            local.markSynced(serverVersion: Int32(version))
        } else if version > 0 {
            local.serverVersion = Int32(version)
            if local.syncStatusEnum == .pendingCreate && modifiedDuringSync {
                local.syncStatusEnum = .pendingUpdate
            }
        }
    }
}
