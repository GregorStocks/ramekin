-- Remove soft-delete: hard-delete any soft-deleted rows first
DELETE FROM recipe_version_tags
    WHERE tag_id IN (SELECT id FROM user_tags WHERE deleted_at IS NOT NULL);
DELETE FROM user_tags WHERE deleted_at IS NOT NULL;

ALTER TABLE user_tags
    DROP COLUMN IF EXISTS deleted_at;
