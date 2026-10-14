-- The whole schema on SQLite. The Postgres migration history it replaces
-- lives in git; `ramekin-server import-postgres` copies an existing
-- Postgres database into a fresh file built from this baseline.
--
-- Conventions:
-- - ids are UUID text, generated in Rust. No id column has a default, so an
--   insert that forgets one fails instead of writing a NULL key.
-- - Timestamps are UTC text, `YYYY-MM-DD HH:MM:SS.SSS+00:00`, matching
--   `db::now()` and Diesel's own encoding, so they compare as text.
-- - Case-insensitive names (usernames, tag names) carry a generated key
--   column, fold_case(name): the search contract's lowercase mapping, a
--   function the server registers on every connection (SQLite's own lower()
--   only folds ASCII).

CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    username_key TEXT NOT NULL GENERATED ALWAYS AS (fold_case(username)) STORED,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    deleted_at TEXT
);
CREATE UNIQUE INDEX idx_users_username_key ON users (username_key) WHERE deleted_at IS NULL;

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    token_type TEXT NOT NULL DEFAULT 'session'
);
CREATE INDEX idx_sessions_user_id ON sessions (user_id);

CREATE TABLE client_log_uploads (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    platform TEXT NOT NULL,
    app_version TEXT,
    os_info TEXT,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    deleted_at TEXT
);
CREATE INDEX idx_client_log_uploads_user_id ON client_log_uploads (user_id) WHERE deleted_at IS NULL;

CREATE TABLE data_migrations (
    name TEXT PRIMARY KEY NOT NULL,
    run_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now'))
);

CREATE TABLE ingredient_name_resolutions (
    name TEXT PRIMARY KEY NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'resolved', 'failed')),
    disposition TEXT CHECK (disposition IN ('entry', 'estimate', 'not_food', 'unknown')),
    catalog_key TEXT,
    model TEXT,
    error TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    kcal_per_100g REAL CHECK (kcal_per_100g >= 0),
    grams_per_cup REAL CHECK (grams_per_cup > 0),
    grams_per_piece REAL CHECK (grams_per_piece > 0),
    CHECK ((status = 'resolved') = (disposition IS NOT NULL)),
    CHECK ((disposition IS 'entry') = (catalog_key IS NOT NULL)),
    CHECK ((status = 'failed') = (error IS NOT NULL)),
    CHECK ((disposition IS 'estimate') = (kcal_per_100g IS NOT NULL)),
    CHECK (disposition IS 'estimate' OR (grams_per_cup IS NULL AND grams_per_piece IS NULL))
);
CREATE INDEX ingredient_name_resolutions_open ON ingredient_name_resolutions (status)
    WHERE status IN ('pending', 'failed');

CREATE TABLE photos (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    content_type TEXT NOT NULL,
    data BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    deleted_at TEXT,
    thumbnail BLOB NOT NULL,
    width INTEGER,
    height INTEGER,
    file_size INTEGER
);
CREATE INDEX idx_photos_user_id ON photos (user_id);

CREATE TABLE photo_thumbnails (
    id TEXT PRIMARY KEY NOT NULL,
    photo_id TEXT NOT NULL REFERENCES photos (id),
    size INTEGER NOT NULL,
    data BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    UNIQUE (photo_id, size)
);

-- One row holding the last change stamp handed out. Triggers below stamp
-- every sync-visible write with the next value. SQLite runs one write
-- transaction at a time, so stamps commit in order and a sync can use the
-- current value as an exact watermark.
CREATE TABLE sync_clock (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    value INTEGER NOT NULL
);
INSERT INTO sync_clock (id, value) VALUES (1, 0);

-- `seq` is a stable integer key for the search index (FTS5 rows need an
-- integer rowid, and VACUUM may renumber an implicit one).
CREATE TABLE recipes (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL REFERENCES users (id),
    current_version_id TEXT REFERENCES recipe_versions (id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    deleted_at TEXT,
    -- The sync_clock stamp of the soft delete, set by recipes_stamp_delete.
    deleted_seq INTEGER
);
CREATE INDEX idx_recipes_created_at ON recipes (created_at);
CREATE INDEX idx_recipes_user_active ON recipes (user_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_recipes_user_deleted_seq ON recipes (user_id, deleted_seq) WHERE deleted_at IS NOT NULL;

-- Versions are immutable once written; edits add a version and repoint
-- recipes.current_version_id.
CREATE TABLE recipe_versions (
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
    version_source TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    change_seq INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_recipe_versions_recipe_id ON recipe_versions (recipe_id);

CREATE TABLE user_tags (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    name TEXT NOT NULL,
    name_key TEXT NOT NULL GENERATED ALWAYS AS (fold_case(name)) STORED,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    deleted_at TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    change_seq INTEGER NOT NULL DEFAULT 0,
    UNIQUE (user_id, name_key)
);
CREATE INDEX idx_user_tags_user_change_seq ON user_tags (user_id, change_seq);

CREATE TABLE recipe_version_tags (
    recipe_version_id TEXT NOT NULL REFERENCES recipe_versions (id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES user_tags (id) ON DELETE CASCADE,
    PRIMARY KEY (recipe_version_id, tag_id)
) WITHOUT ROWID;
CREATE INDEX idx_recipe_version_tags_tag ON recipe_version_tags (tag_id);

CREATE TABLE meal_plans (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    recipe_id TEXT NOT NULL REFERENCES recipes (id),
    meal_date TEXT NOT NULL,
    meal_type TEXT NOT NULL CHECK (meal_type IN ('breakfast', 'lunch', 'dinner', 'snack')),
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    deleted_at TEXT
);
CREATE INDEX idx_meal_plans_recipe ON meal_plans (recipe_id) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX idx_meal_plans_unique_slot_active
    ON meal_plans (user_id, meal_date, meal_type, recipe_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_meal_plans_user_date ON meal_plans (user_id, meal_date) WHERE deleted_at IS NULL;

CREATE TABLE shopping_list_items (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    item TEXT NOT NULL,
    amount TEXT,
    note TEXT,
    source_recipe_id TEXT REFERENCES recipes (id) ON DELETE SET NULL,
    source_recipe_title TEXT,
    is_checked INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    client_id TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    deleted_at TEXT,
    category_override TEXT,
    change_seq INTEGER NOT NULL DEFAULT 0,
    computed_category TEXT,
    UNIQUE (user_id, client_id)
);
CREATE INDEX idx_shopping_list_items_user_change_seq ON shopping_list_items (user_id, change_seq);
CREATE INDEX idx_shopping_list_user ON shopping_list_items (user_id, sort_order);

CREATE TABLE scrape_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    url TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    failed_at_step TEXT,
    recipe_id TEXT,
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    current_step TEXT,
    photo_only INTEGER NOT NULL DEFAULT 0,
    current_step_started_at TEXT,
    expected_version_id TEXT REFERENCES recipe_versions (id),
    idempotency_key TEXT
);
CREATE INDEX idx_scrape_jobs_status ON scrape_jobs (status);
CREATE INDEX idx_scrape_jobs_user_id ON scrape_jobs (user_id);
CREATE UNIQUE INDEX scrape_jobs_user_idempotency_key ON scrape_jobs (user_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE TABLE step_outputs (
    id TEXT PRIMARY KEY NOT NULL,
    scrape_job_id TEXT NOT NULL REFERENCES scrape_jobs (id),
    step_name TEXT NOT NULL,
    build_id TEXT NOT NULL,
    output TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    duration_ms INTEGER,
    summary TEXT,
    success INTEGER NOT NULL DEFAULT 1,
    error TEXT
);
CREATE INDEX step_outputs_scrape_job_id_idx ON step_outputs (scrape_job_id);

-- Sync stamps. Each trigger takes the next clock value and writes it to the
-- row. The UPDATE triggers fire only when change_seq itself didn't change,
-- so the stamping UPDATE doesn't retrigger them (and an explicit restamp
-- passes through).

CREATE TRIGGER recipe_versions_stamp AFTER INSERT ON recipe_versions
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE recipe_versions SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;

CREATE TRIGGER user_tags_stamp_insert AFTER INSERT ON user_tags
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE user_tags SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;

CREATE TRIGGER user_tags_stamp_update AFTER UPDATE ON user_tags
WHEN NEW.change_seq IS OLD.change_seq
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE user_tags SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;

CREATE TRIGGER shopping_list_items_stamp_insert AFTER INSERT ON shopping_list_items
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE shopping_list_items SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;

CREATE TRIGGER shopping_list_items_stamp_update AFTER UPDATE ON shopping_list_items
WHEN NEW.change_seq IS OLD.change_seq
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE shopping_list_items SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;

CREATE TRIGGER recipes_stamp_delete AFTER UPDATE OF deleted_at ON recipes
WHEN NEW.deleted_at IS NOT NULL AND OLD.deleted_at IS NULL
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE recipes SET deleted_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE seq = NEW.seq;
END;

-- Recipe text search: one row per live recipe holding its current version's
-- searchable fields, normalized with normalize_for_search (a function the
-- server registers on every connection), keyed by recipes.seq. The trigram
-- tokenizer makes a quoted term a substring match, like the old ILIKE.
CREATE VIRTUAL TABLE recipe_search USING fts5 (
    title,
    description,
    instructions,
    notes,
    ingredients,
    tokenize = 'trigram'
);

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
