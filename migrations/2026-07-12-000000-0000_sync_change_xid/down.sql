DROP INDEX idx_user_tags_user_change_xid;
DROP INDEX idx_recipes_user_deleted_xid;

ALTER TABLE user_tags DROP COLUMN change_xid;

ALTER TABLE recipes DROP CONSTRAINT recipes_deleted_xid_matches_deleted_at;
ALTER TABLE recipes DROP COLUMN deleted_xid;

ALTER TABLE recipe_versions DROP COLUMN change_xid;

DROP FUNCTION change_xid_watermark();
DROP FUNCTION current_change_xid();
