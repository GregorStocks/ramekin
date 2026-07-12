-- Race-safe change cursor for GET /api/recipes/sync.
--
-- A wall-clock cursor cannot order changes against commits: a transaction takes
-- its `NOW()` (= transaction start) before the sync SELECT's snapshot but can
-- commit after it. The row is invisible to that sync, and the next sync's
-- `> last_sync_at` filter excludes it forever.
--
-- Instead, stamp every sync-visible change with the 64-bit id of the
-- transaction that wrote it, and cursor on the snapshot's xmin: the lowest
-- transaction id still in flight. Every transaction not yet committed-and-
-- visible has an id >= that watermark, so it is picked up by the next sync
-- rather than skipped.

-- The writing transaction's own id. Assigned at the transaction's first write,
-- so a row's stamp is the same id the tuple itself is written under.
CREATE FUNCTION current_change_xid()
RETURNS BIGINT
LANGUAGE sql
VOLATILE
PARALLEL SAFE
AS $$ SELECT pg_current_xact_id()::text::bigint $$;

-- The lowest transaction id still in flight for the current snapshot. Every
-- change stamped below this is settled: committed (and therefore visible) or
-- aborted. Reading this does not assign an xid to the calling transaction.
CREATE FUNCTION change_xid_watermark()
RETURNS BIGINT
LANGUAGE sql
VOLATILE
PARALLEL SAFE
AS $$ SELECT pg_snapshot_xmin(pg_current_snapshot())::text::bigint $$;

-- Recipe creates and updates both insert a new recipe_versions row, so one
-- stamp on the version covers both.
ALTER TABLE recipe_versions
ADD COLUMN change_xid BIGINT NOT NULL DEFAULT current_change_xid();

-- Soft-delete tombstones. Set alongside deleted_at, which the CHECK enforces.
ALTER TABLE recipes
ADD COLUMN deleted_xid BIGINT;

UPDATE recipes SET deleted_xid = current_change_xid() WHERE deleted_at IS NOT NULL;

ALTER TABLE recipes
ADD CONSTRAINT recipes_deleted_xid_matches_deleted_at
CHECK ((deleted_at IS NULL) = (deleted_xid IS NULL));

-- A tag rename or delete changes the rendered tags of every recipe carrying it,
-- so tag mutations are part of the recipe change feed.
ALTER TABLE user_tags
ADD COLUMN change_xid BIGINT NOT NULL DEFAULT current_change_xid();

CREATE INDEX idx_recipes_user_deleted_xid
ON recipes (user_id, deleted_xid)
WHERE deleted_at IS NOT NULL;

CREATE INDEX idx_user_tags_user_change_xid
ON user_tags (user_id, change_xid);
