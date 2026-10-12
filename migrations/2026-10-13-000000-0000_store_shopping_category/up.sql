-- The shopping-list category computed from an item's name, stored when the
-- item is written instead of recomputed on every read. A catalog deploy no
-- longer touches every live item (via categorizer_version, now unused) to
-- push new categories through incremental sync; an item is recategorized when
-- it's written or its name gets resolved. The server backfills NULLs at
-- startup, since the categorizer is Rust.
ALTER TABLE shopping_list_items ADD COLUMN computed_category TEXT;
