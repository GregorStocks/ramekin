DROP INDEX idx_shopping_list_items_user_change_xid;
DROP TRIGGER shopping_list_items_stamp_change_xid ON shopping_list_items;
DROP FUNCTION stamp_change_xid();
ALTER TABLE shopping_list_items DROP COLUMN change_xid;
