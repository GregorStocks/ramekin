ALTER TABLE shopping_list_items
    ADD COLUMN deleted_at TIMESTAMPTZ;

CREATE UNIQUE INDEX idx_shopping_list_client_id
    ON shopping_list_items(user_id, client_id)
    WHERE client_id IS NOT NULL;

CREATE INDEX idx_shopping_list_deleted
    ON shopping_list_items(user_id, deleted_at)
    WHERE deleted_at IS NOT NULL;
