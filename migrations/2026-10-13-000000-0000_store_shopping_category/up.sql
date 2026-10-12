-- The shopping-list category computed from an item's name, stored when the
-- item is written instead of recomputed on every read. A catalog deploy no
-- longer touches every live item (via categorizer_version, now unused) to
-- push new categories through incremental sync: startup stores each item's
-- category and touches only the items whose category changed, which also
-- fills these NULLs, since the categorizer is Rust.
ALTER TABLE shopping_list_items ADD COLUMN computed_category TEXT;
