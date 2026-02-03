DROP INDEX IF EXISTS idx_shopping_list_deleted;
ALTER TABLE shopping_list_items
    DROP CONSTRAINT IF EXISTS uq_shopping_list_client_id;

ALTER TABLE shopping_list_items
    DROP COLUMN IF EXISTS deleted_at;
