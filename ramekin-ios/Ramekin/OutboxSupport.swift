import CoreData
import Foundation

/// Where a cached row stands in its domain's outbox (doc/sync-design.md).
enum OutboxStatus: String {
    case synced
    case pendingCreate = "pending_create"
    case pendingUpdate = "pending_update"
    case pendingDelete = "pending_delete"
    /// Undeleting a row (shopping list only): a restore op carrying every
    /// field, which also recreates a row whose create never reached the
    /// server.
    case pendingRestore = "pending_restore"
}

/// A cached row whose local writes wait in an outbox until a sync sends them
/// as field ops: `ShoppingItem` and `CachedMealPlan`.
protocol OutboxRow: NSManagedObject {
    /// The row's `id` (named apart from `Identifiable.id`, which
    /// `NSManagedObject` also has).
    var rowId: UUID? { get set }
    var syncStatus: String? { get set }
    var dirtyFields: String? { get set }
    var changeSeq: Int64 { get set }
    /// API names of the fields an update op can send.
    static var syncedFields: [String] { get }
    /// A field's current value, to tell whether it changed while a sync
    /// carrying it was in flight.
    func syncedValue(of field: String) -> String?
}

/// An op as sent: its kind, and the value of each field it carried, so a
/// field edited again mid-sync stays dirty.
struct OutboxSent {
    let kind: OpKind
    let values: [String: String?]
}

enum OutboxSettled: Equatable {
    case kept
    /// The server deleted the row, or the local delete went through.
    case removed
}

extension OutboxRow {
    var syncStatusEnum: OutboxStatus {
        get { OutboxStatus(rawValue: syncStatus ?? "synced") ?? .synced }
        set { syncStatus = newValue.rawValue }
    }

    var pendingWrite: SyncApplySupport.PendingWrite? {
        switch syncStatusEnum {
        case .synced: return nil
        case .pendingCreate: return .create
        // A full resync keeps it, as it does an update, until its op settles.
        case .pendingUpdate, .pendingRestore: return .update
        case .pendingDelete: return .delete
        }
    }

    /// The fields with unsent local edits: every field of a pending create
    /// or restore, none of a synced row or pending delete.
    var dirtyFieldSet: Set<String> {
        get {
            switch syncStatusEnum {
            case .pendingCreate, .pendingRestore:
                return Set(Self.syncedFields)
            case .pendingUpdate:
                return DirtyFields.decode(dirtyFields, allFields: Self.syncedFields)
            case .synced, .pendingDelete:
                return []
            }
        }
        set { dirtyFields = DirtyFields.encode(newValue) }
    }

    /// Marks `fields` as edited locally, to send on the next sync.
    func markUpdated(_ fields: Set<String>) {
        switch syncStatusEnum {
        case .synced:
            dirtyFieldSet = fields
            syncStatusEnum = .pendingUpdate
        case .pendingUpdate:
            dirtyFieldSet = dirtyFieldSet.union(fields)
        case .pendingCreate, .pendingRestore, .pendingDelete:
            break
        }
    }

    /// Marks the row for deletion. Even a row whose create looks unsent is
    /// deleted through the server: the create may have reached it on a sync
    /// whose response was lost.
    func markDeleted() {
        dirtyFields = nil
        syncStatusEnum = .pendingDelete
    }

    /// Marks a deleted row (or one whose delete is still pending) to be
    /// undeleted on the next sync.
    func markRestored() {
        dirtyFields = nil
        syncStatusEnum = .pendingRestore
    }

    /// Marks the row as matching the server.
    func markSynced() {
        dirtyFields = nil
        syncStatusEnum = .synced
    }

    /// What an op of `kind` for this row sends.
    func sent(_ kind: OpKind) -> OutboxSent {
        OutboxSent(
            kind: kind,
            values: Dictionary(uniqueKeysWithValues: dirtyFieldSet.map { ($0, syncedValue(of: $0)) })
        )
    }

    /// Settle the row's op from its result. A field that still holds the
    /// value sent is clean; one edited again while the sync was in flight
    /// stays dirty and goes out next round, and so does a delete made then.
    /// A refused op settles the same way: its sent fields are clean, so the
    /// server's copy (or tombstone) in the feed replaces them, and fields
    /// edited since stay dirty.
    func settle(result: OpResult, sent: OutboxSent) -> OutboxSettled {
        if sent.kind == .delete {
            // Restored while its delete was in flight: the restore goes next,
            // even when the delete was gone (its create never arrived, which
            // the restore's fields make up for).
            return syncStatusEnum == .pendingRestore ? .kept : .removed
        }
        if result == .gone {
            return .removed
        }
        if result != .applied, sent.kind == .create, syncStatusEnum == .pendingCreate,
           dirtyFieldSet.contains(where: { syncedValue(of: $0) != sent.values[$0] ?? nil }) {
            // Edited while its refused create was in flight (moved to
            // another meal, say): retry it as a new create. The fresh id
            // keeps the refused one's tombstone from dropping it.
            rowId = UUID()
            return .kept
        }
        switch syncStatusEnum {
        case .pendingDelete, .synced:
            return .kept
        case .pendingRestore where sent.kind != .restore:
            // Restored while an update was in flight: the restore goes next.
            return .kept
        case .pendingCreate, .pendingUpdate, .pendingRestore:
            let stillDirty = dirtyFieldSet.filter { field in
                guard let value = sent.values[field] else { return true }
                return syncedValue(of: field) != value
            }
            if stillDirty.isEmpty {
                markSynced()
            } else {
                syncStatusEnum = .pendingUpdate
                dirtyFieldSet = stillDirty
            }
            return .kept
        }
    }
}

/// A row in a sync response's `changes`.
protocol SyncedChange {
    var id: UUID { get }
    var changeSeq: Int64 { get }
}

/// A writable domain's sync response.
protocol OutboxResponse {
    associatedtype Change: SyncedChange
    var changes: [Change] { get }
    var deleted: [UUID] { get }
    var fullResync: Bool { get }
}

extension SyncShoppingListItem: SyncedChange {}
extension SyncShoppingListResponse: OutboxResponse {}
extension SyncMealPlan: SyncedChange {}
extension SyncMealPlansResponse: OutboxResponse {}

enum OutboxSupport {
    /// Apply a response's changes and tombstones to every cached row of the
    /// account (see `SyncApplySupport.plan`). `take` writes a server row over
    /// the fields that aren't dirty; `insert` makes a row for one the cache
    /// lacks.
    static func apply<Response: OutboxResponse, Row: OutboxRow>(
        _ response: Response,
        cached: [Row],
        in context: NSManagedObjectContext,
        insert: (UUID) -> Row,
        take: (Response.Change, Row) -> Void
    ) {
        var byId: [UUID: Row] = [:]
        for row in cached {
            if let rowId = row.rowId { byId[rowId] = row }
        }
        let changesById = Dictionary(response.changes.map { ($0.id, $0) }, uniquingKeysWith: { _, last in last })
        // A pending restore outranks a tombstone (its own delete's, say):
        // the restore op settles the row.
        let restoring = Set(byId.filter { $0.value.syncStatusEnum == .pendingRestore }.keys)
        let actions = SyncApplySupport.plan(
            cached: byId.mapValues { SyncApplySupport.CachedState(changeSeq: $0.changeSeq, pending: $0.pendingWrite) },
            changes: response.changes.map { (id: $0.id, changeSeq: $0.changeSeq) },
            deleted: response.deleted.filter { !restoring.contains($0) },
            fullResync: response.fullResync
        )
        for (rowId, action) in actions {
            switch action {
            case .keep:
                break
            case .drop:
                if let row = byId[rowId] { context.delete(row) }
            case .take:
                guard let change = changesById[rowId] else { continue }
                let row = byId[rowId] ?? insert(rowId)
                take(change, row)
                row.changeSeq = change.changeSeq
            }
        }
    }
}
