-- Extensions for composite GIN indexes with user_id + text search
CREATE EXTENSION IF NOT EXISTS btree_gin;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Composite indexes: user_id + text search (partial, excludes deleted)
-- These allow single index scan for "user's recipes matching search term"
CREATE INDEX idx_recipes_user_title_search
ON recipes USING GIN (user_id, title gin_trgm_ops)
WHERE deleted_at IS NULL;

CREATE INDEX idx_recipes_user_description_search
ON recipes USING GIN (user_id, description gin_trgm_ops)
WHERE deleted_at IS NULL;

-- Replace existing tags index with composite user_id + tags
DROP INDEX IF EXISTS idx_recipes_tags;
CREATE INDEX idx_recipes_user_tags
ON recipes USING GIN (user_id, tags)
WHERE deleted_at IS NULL;
