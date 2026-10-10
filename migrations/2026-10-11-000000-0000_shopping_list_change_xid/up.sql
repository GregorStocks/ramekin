-- Race-safe change cursor for POST /api/shopping-list/sync, mirroring the
-- recipe sync cursor from the `sync_change_xid` migration.
--
-- Writers stamp `updated_at` from the clock before they commit, so a row
-- stamped before a concurrent sync's `sync_timestamp` but committed after that
-- sync's read is invisible to it and excluded by the next sync's
-- `> last_sync_at` filter forever. Stamping each change with its transaction
-- id and cursoring on the snapshot's xmin redelivers such rows instead.

ALTER TABLE shopping_list_items
ADD COLUMN change_xid BIGINT NOT NULL DEFAULT current_change_xid();

-- Every UPDATE is a sync-visible change (edits, soft deletes, the categorizer
-- version bump, the ingredient-name resolver's touch), so stamp them all here
-- rather than trusting each writer to remember. Generic, so other change-feed
-- tables can attach it too.
CREATE FUNCTION stamp_change_xid()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.change_xid := current_change_xid();
    RETURN NEW;
END
$$;

CREATE TRIGGER shopping_list_items_stamp_change_xid
BEFORE UPDATE ON shopping_list_items
FOR EACH ROW
EXECUTE FUNCTION stamp_change_xid();

CREATE INDEX idx_shopping_list_items_user_change_xid
ON shopping_list_items (user_id, change_xid);
