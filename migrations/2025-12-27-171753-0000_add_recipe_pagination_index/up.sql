-- Composite index for paginated recipe queries
-- Supports filtering by user_id, deleted_at, and ordering by updated_at DESC
CREATE INDEX idx_recipes_user_id_updated_at
ON recipes(user_id, updated_at DESC, deleted_at)
WHERE deleted_at IS NULL;
