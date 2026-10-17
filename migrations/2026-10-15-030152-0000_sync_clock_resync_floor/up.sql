-- The clock value of the last `db::restamp_live` (a restore or an import).
-- A cursor at or below it was issued before the database's contents changed
-- under clients' feet, so it gets a flagged full resync instead of a delta:
-- rows the change rolled back have no tombstone to delete them by.
ALTER TABLE sync_clock ADD COLUMN resync_floor INTEGER NOT NULL DEFAULT 0;
