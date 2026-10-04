-- The categorizer (catalog plus keyword rules) version that last computed the
-- item's category. At startup, items from another version are touched so
-- incremental sync sends their recomputed category.
ALTER TABLE shopping_list_items ADD COLUMN categorizer_version TEXT;
