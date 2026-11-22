-- recipes: a recipe is soft-deleted exactly when it has a deletion stamp.
-- The 2026-11-07 CHECK only pinned one direction (a stamp implies
-- deleted_at), because the recipes_stamp_delete trigger set the stamp after
-- the UPDATE that set deleted_at, leaving the row briefly unstamped. The
-- soft delete now takes the sync_clock stamp itself and writes both columns
-- in one UPDATE (api/recipes/delete.rs), so the trigger goes and the CHECK
-- covers both directions.
--
-- Like that migration, this rebuilds a table other tables reference, so it
-- runs outside Diesel's transaction (metadata.toml) with foreign keys off
-- and a transaction of its own. Every row is copied as it is.
PRAGMA foreign_keys = OFF;
BEGIN IMMEDIATE;

-- A deleted recipe without a stamp (the trigger should have left none) gets
-- a fresh one, so clients syncing after this learn of the deletion.
UPDATE sync_clock SET value = value + 1
WHERE id = 1
  AND EXISTS (SELECT 1 FROM recipes WHERE deleted_at IS NOT NULL AND deleted_seq IS NULL);
UPDATE recipes SET deleted_seq = (SELECT value FROM sync_clock WHERE id = 1)
WHERE deleted_at IS NOT NULL AND deleted_seq IS NULL;

-- `seq` is copied, so the search index's rowids still match.
CREATE TABLE recipes_new (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL REFERENCES users (id),
    current_version_id TEXT REFERENCES recipe_versions (id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    deleted_at TEXT,
    -- The sync_clock stamp of the soft delete, written with deleted_at.
    deleted_seq INTEGER,
    CHECK ((deleted_at IS NULL) = (deleted_seq IS NULL))
);
INSERT INTO recipes_new (seq, id, user_id, current_version_id, created_at, deleted_at, deleted_seq)
SELECT seq, id, user_id, current_version_id, created_at, deleted_at, deleted_seq FROM recipes;
DROP TABLE recipes;
ALTER TABLE recipes_new RENAME TO recipes;
CREATE INDEX idx_recipes_created_at ON recipes (created_at);
CREATE INDEX idx_recipes_user_active ON recipes (user_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_recipes_user_deleted_seq ON recipes (user_id, deleted_seq) WHERE deleted_at IS NOT NULL;

-- Dropping the table dropped its triggers; recipes_stamp_delete stays gone.
CREATE TRIGGER recipes_search_repoint AFTER UPDATE OF current_version_id ON recipes
WHEN NEW.deleted_at IS NULL
BEGIN
    DELETE FROM recipe_search WHERE rowid = NEW.seq;
    INSERT INTO recipe_search (rowid, title, description, instructions, notes, ingredients)
    SELECT NEW.seq,
           normalize_for_search(v.title),
           normalize_for_search(v.description),
           normalize_for_search(v.instructions),
           normalize_for_search(v.notes),
           normalize_for_search(v.ingredients)
    FROM recipe_versions v
    WHERE v.id = NEW.current_version_id;
END;

CREATE TRIGGER recipes_search_delete AFTER UPDATE OF deleted_at ON recipes
WHEN NEW.deleted_at IS NOT NULL
BEGIN
    DELETE FROM recipe_search WHERE rowid = NEW.seq;
END;

COMMIT;
PRAGMA foreign_keys = ON;
