-- CHECK constraints for columns that encode a state or pair with another
-- column. SQLite can only add a CHECK by rebuilding the table, and dropping
-- a table other tables reference with foreign keys on would run their ON
-- DELETE actions (recipe_version_tags cascades, shopping_list_items sets
-- source_recipe_id to null). So this migration runs outside Diesel's
-- transaction (metadata.toml), turns foreign keys off, and does the
-- rebuilds in a transaction of its own, following SQLite's "making other
-- kinds of table schema changes" procedure. Every row is copied as it is,
-- after the backfills below put rows that would break a new constraint into
-- a valid state; nothing is deleted.
PRAGMA foreign_keys = OFF;
BEGIN IMMEDIATE;

-- sessions.token_type: a login session or a bookmarklet token. Only those
-- two have ever been written; anything else gets the narrower scope.
UPDATE sessions SET token_type = 'bookmarklet'
WHERE token_type NOT IN ('session', 'bookmarklet');

CREATE TABLE sessions_new (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    token_type TEXT NOT NULL DEFAULT 'session'
        CHECK (token_type IN ('session', 'bookmarklet'))
);
INSERT INTO sessions_new (id, user_id, token_hash, expires_at, created_at, token_type)
SELECT id, user_id, token_hash, expires_at, created_at, token_type FROM sessions;
DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;
CREATE INDEX idx_sessions_user_id ON sessions (user_id);

-- step_outputs: a failed step says why and a successful one has no error;
-- an output kept inline has no output_hash (NULL now, rather than ''), and
-- one in the file store holds JSON null inline.
UPDATE step_outputs SET error = 'The step failed without an error message.'
WHERE success = 0 AND error IS NULL;
-- No writer has ever stored an error with a successful step, and every
-- reader goes by `success`.
UPDATE step_outputs SET error = NULL WHERE success = 1 AND error IS NOT NULL;
UPDATE step_outputs SET success = 1 WHERE success NOT IN (0, 1);
-- A row with a file and its output still inline: the inline copy is whole,
-- so read it rather than the file (which stays in the store).
UPDATE step_outputs SET output_hash = '' WHERE output_hash <> '' AND output <> 'null';

CREATE TABLE step_outputs_new (
    id TEXT PRIMARY KEY NOT NULL,
    scrape_job_id TEXT NOT NULL REFERENCES scrape_jobs (id),
    step_name TEXT NOT NULL,
    build_id TEXT NOT NULL,
    output TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    duration_ms INTEGER,
    summary TEXT,
    success INTEGER NOT NULL DEFAULT 1 CHECK (success IN (0, 1)),
    error TEXT,
    -- The SHA-256 of the file in the store (server/src/files.rs) holding a
    -- large output, or NULL for an output kept inline.
    output_hash TEXT CHECK (output_hash IS NULL OR output = 'null'),
    CHECK ((success = 0) = (error IS NOT NULL))
);
INSERT INTO step_outputs_new (
    id, scrape_job_id, step_name, build_id, output, created_at, duration_ms, summary,
    success, error, output_hash
)
SELECT id, scrape_job_id, step_name, build_id, output, created_at, duration_ms, summary,
       success, error, NULLIF(output_hash, '')
FROM step_outputs;
DROP TABLE step_outputs;
ALTER TABLE step_outputs_new RENAME TO step_outputs;
CREATE INDEX step_outputs_scrape_job_id_idx ON step_outputs (scrape_job_id);

-- recipe_versions.version_source: what wrote the version. Sources from
-- before the normalize ops registry move to 'normalize' with the op they
-- ran, as 2026-10-25-000000-0000_normalize_ops did for most of them;
-- 'reparse' and 'migration' versions were written by one-off jobs since
-- removed, and stay as they are.
UPDATE recipe_versions SET version_source = 'normalize', normalize_ops = '["normalize_title"]'
WHERE version_source = 'normalize_title';
UPDATE recipe_versions SET version_source = 'normalize', normalize_ops = '["generate_description"]'
WHERE version_source = 'generate_description';
UPDATE recipe_versions SET version_source = 'normalize', normalize_ops = '["auto_tag"]'
WHERE version_source IN ('enrichment', 'auto_tag');
-- Nothing else has ever been written; should something be, it reads as a
-- version a migration wrote.
UPDATE recipe_versions SET version_source = 'migration'
WHERE version_source NOT IN (
    'user', 'scrape', 'rescrape', 'import', 'photo_import', 'photo_rescrape',
    'normalize', 'ai_edit', 'ai_photo', 'reparse', 'migration'
);

-- A rename checks every trigger in the schema, and this one on recipes
-- reads recipe_versions, which is gone between its drop and the rename.
-- The recipes rebuild below creates it again.
DROP TRIGGER recipes_search_repoint;

CREATE TABLE recipe_versions_new (
    id TEXT PRIMARY KEY NOT NULL,
    recipe_id TEXT NOT NULL REFERENCES recipes (id),
    title TEXT NOT NULL,
    description TEXT,
    ingredients TEXT NOT NULL,
    instructions TEXT NOT NULL,
    source_url TEXT,
    source_name TEXT,
    -- JSON array of photo ids, in display order.
    photo_ids TEXT NOT NULL DEFAULT '[]',
    servings TEXT,
    prep_time TEXT,
    cook_time TEXT,
    total_time TEXT,
    rating INTEGER,
    difficulty TEXT,
    nutritional_info TEXT,
    notes TEXT,
    -- What wrote the version.
    version_source TEXT NOT NULL CHECK (version_source IN (
        'user', 'scrape', 'rescrape', 'import', 'photo_import', 'photo_rescrape',
        'normalize', 'ai_edit', 'ai_photo', 'reparse', 'migration'
    )),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    change_seq INTEGER NOT NULL DEFAULT 0,
    normalize_ops TEXT,
    kcal_per_serving REAL,
    kcal_per_serving_label TEXT
        CHECK ((kcal_per_serving IS NULL) = (kcal_per_serving_label IS NULL))
);
INSERT INTO recipe_versions_new (
    id, recipe_id, title, description, ingredients, instructions, source_url, source_name,
    photo_ids, servings, prep_time, cook_time, total_time, rating, difficulty,
    nutritional_info, notes, version_source, created_at, change_seq, normalize_ops,
    kcal_per_serving, kcal_per_serving_label
)
SELECT id, recipe_id, title, description, ingredients, instructions, source_url, source_name,
       photo_ids, servings, prep_time, cook_time, total_time, rating, difficulty,
       nutritional_info, notes, version_source, created_at, change_seq, normalize_ops,
       kcal_per_serving, kcal_per_serving_label
FROM recipe_versions;
DROP TABLE recipe_versions;
ALTER TABLE recipe_versions_new RENAME TO recipe_versions;
CREATE INDEX idx_recipe_versions_recipe_id ON recipe_versions (recipe_id);

CREATE TRIGGER recipe_versions_stamp AFTER INSERT ON recipe_versions
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE recipe_versions SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;

-- recipes.deleted_seq: the stamp of a soft delete, so only a deleted recipe
-- has one. (The converse can't be a CHECK: recipes_stamp_delete sets the
-- stamp after the UPDATE that sets deleted_at, so that row briefly has
-- deleted_at without deleted_seq.) Recipes are never restored, so a live
-- recipe's stamp means nothing.
UPDATE recipes SET deleted_seq = NULL WHERE deleted_at IS NULL AND deleted_seq IS NOT NULL;

-- `seq` is copied, so the search index's rowids still match.
CREATE TABLE recipes_new (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL REFERENCES users (id),
    current_version_id TEXT REFERENCES recipe_versions (id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    deleted_at TEXT,
    -- The sync_clock stamp of the soft delete, set by recipes_stamp_delete.
    deleted_seq INTEGER CHECK (deleted_seq IS NULL OR deleted_at IS NOT NULL)
);
INSERT INTO recipes_new (seq, id, user_id, current_version_id, created_at, deleted_at, deleted_seq)
SELECT seq, id, user_id, current_version_id, created_at, deleted_at, deleted_seq FROM recipes;
DROP TABLE recipes;
ALTER TABLE recipes_new RENAME TO recipes;
CREATE INDEX idx_recipes_created_at ON recipes (created_at);
CREATE INDEX idx_recipes_user_active ON recipes (user_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_recipes_user_deleted_seq ON recipes (user_id, deleted_seq) WHERE deleted_at IS NOT NULL;

CREATE TRIGGER recipes_stamp_delete AFTER UPDATE OF deleted_at ON recipes
WHEN NEW.deleted_at IS NOT NULL AND OLD.deleted_at IS NULL
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE recipes SET deleted_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE seq = NEW.seq;
END;

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

-- shopping_list_history: an undo pairs two entries, the undo naming the
-- entry it undid (undoes_id) and that entry naming its undo (undone_by_id).
-- An entry is undone at most once and undoes at most one entry, and never
-- itself. Both halves are written in one transaction, so every pair should
-- already match; any that don't are rebuilt from undoes_id, since the undo
-- entry holds the changes that undid the other.
UPDATE shopping_list_history SET undoes_id = NULL WHERE undoes_id = id;
UPDATE shopping_list_history SET undone_by_id = NULL WHERE undone_by_id = id;
UPDATE shopping_list_history AS undone SET undone_by_id = (
    SELECT MIN(undo.id) FROM shopping_list_history AS undo WHERE undo.undoes_id = undone.id
)
WHERE EXISTS (SELECT 1 FROM shopping_list_history AS undo WHERE undo.undoes_id = undone.id);
-- Later undos of an entry already undone (none can be made: the undo
-- endpoint checks first) keep their changes but no longer claim the entry.
UPDATE shopping_list_history AS undo SET undoes_id = NULL
WHERE undoes_id IS NOT NULL
  AND id IS NOT (SELECT undone.undone_by_id FROM shopping_list_history AS undone
                 WHERE undone.id = undo.undoes_id);
UPDATE shopping_list_history AS undone SET undone_by_id = NULL
WHERE undone_by_id IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM shopping_list_history AS undo
                  WHERE undo.id = undone.undone_by_id AND undo.undoes_id = undone.id);

CREATE TABLE shopping_list_history_new (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    -- The entry this one undid, and the entry that undid this one.
    undoes_id TEXT REFERENCES shopping_list_history (id) CHECK (undoes_id IS NOT id),
    undone_by_id TEXT REFERENCES shopping_list_history (id) CHECK (undone_by_id IS NOT id)
);
INSERT INTO shopping_list_history_new (id, user_id, created_at, undoes_id, undone_by_id)
SELECT id, user_id, created_at, undoes_id, undone_by_id FROM shopping_list_history;
DROP TABLE shopping_list_history;
ALTER TABLE shopping_list_history_new RENAME TO shopping_list_history;
CREATE INDEX idx_shopping_list_history_user ON shopping_list_history (user_id, id);
CREATE UNIQUE INDEX idx_shopping_list_history_undoes ON shopping_list_history (undoes_id)
    WHERE undoes_id IS NOT NULL;
CREATE UNIQUE INDEX idx_shopping_list_history_undone_by ON shopping_list_history (undone_by_id)
    WHERE undone_by_id IS NOT NULL;

-- The pairing itself spans two rows, which a CHECK can't see: an entry's
-- undo is set once, to an entry that undoes it.
CREATE TRIGGER shopping_list_history_undo_pairing BEFORE UPDATE OF undone_by_id ON shopping_list_history
WHEN OLD.undone_by_id IS NOT NULL
  OR NOT EXISTS (SELECT 1 FROM shopping_list_history
                 WHERE id = NEW.undone_by_id AND undoes_id = NEW.id)
BEGIN
    SELECT RAISE(ABORT, 'shopping_list_history: an entry is undone once, by an entry that undoes it');
END;

COMMIT;
PRAGMA foreign_keys = ON;
