-- First-class tags: create user_tags table and recipe_version_tags junction table
-- Tags are now stored in normalized tables rather than as an array on recipe_versions

-- Create user_tags table (hard delete - no soft delete for tags)
CREATE TABLE user_tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name CITEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT user_tags_user_name_unique UNIQUE (user_id, name)
);

CREATE INDEX idx_user_tags_user ON user_tags(user_id);

-- Create junction table for recipe_version <-> tag associations
CREATE TABLE recipe_version_tags (
    recipe_version_id UUID NOT NULL REFERENCES recipe_versions(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (recipe_version_id, tag_id)
);

CREATE INDEX idx_recipe_version_tags_tag ON recipe_version_tags(tag_id);

-- Migrate existing data: create user_tags from existing arrays
INSERT INTO user_tags (user_id, name)
SELECT DISTINCT r.user_id, unnest(rv.tags) AS name
FROM recipes r
JOIN recipe_versions rv ON rv.recipe_id = r.id
WHERE r.deleted_at IS NULL
  AND rv.tags IS NOT NULL
  AND array_length(rv.tags, 1) > 0
ON CONFLICT (user_id, name) DO NOTHING;

-- Migrate existing data: populate junction table
INSERT INTO recipe_version_tags (recipe_version_id, tag_id)
SELECT rv.id, ut.id
FROM recipe_versions rv
JOIN recipes r ON r.id = rv.recipe_id
CROSS JOIN LATERAL unnest(rv.tags) AS tag_name
JOIN user_tags ut ON ut.user_id = r.user_id AND ut.name = tag_name
WHERE rv.tags IS NOT NULL AND array_length(rv.tags, 1) > 0;

-- Remove old column and index
DROP INDEX IF EXISTS idx_recipe_versions_tags;
ALTER TABLE recipe_versions DROP COLUMN tags;
