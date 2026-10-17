// Applying a sync response (doc/sync-design.md) to a cached table, pinned
// with iOS by shared-test-vectors/sync-apply.json. The web cache never holds
// unsent local writes, so this is the vectors' clean-row subset.

export interface SyncedRow {
  id: string;
  changeSeq: number;
}

export interface SyncDelta<T extends SyncedRow> {
  changes: T[];
  deleted: string[];
  fullResync: boolean;
}

/**
 * The cache after `response`: tombstones drop rows, a server row replaces
 * the cached one unless it is older, a full resync drops every row it
 * omits, and rows a delta doesn't mention stay.
 */
export function applySyncResponse<T extends SyncedRow>(
  cached: readonly T[],
  response: SyncDelta<T>,
): T[] {
  const deleted = new Set(response.deleted);
  const byId = new Map<string, T>();
  if (!response.fullResync) {
    for (const row of cached) {
      if (!deleted.has(row.id)) byId.set(row.id, row);
    }
  }
  const previous = new Map(cached.map((row) => [row.id, row]));
  for (const change of response.changes) {
    if (deleted.has(change.id)) continue;
    const held = previous.get(change.id);
    byId.set(
      change.id,
      held && held.changeSeq > change.changeSeq ? held : change,
    );
  }
  return Array.from(byId.values());
}
