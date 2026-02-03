ALTER TABLE shopping_list_items
    ADD COLUMN deleted_at TIMESTAMPTZ;

-- Unique constraint for offline sync deduplication.
-- NULLs are treated as distinct in PostgreSQL, so multiple rows with
-- NULL client_id (server-created items) are allowed.
ALTER TABLE shopping_list_items
    ADD CONSTRAINT uq_shopping_list_client_id UNIQUE (user_id, client_id);

CREATE INDEX idx_shopping_list_deleted
    ON shopping_list_items(user_id, deleted_at)
    WHERE deleted_at IS NOT NULL;
