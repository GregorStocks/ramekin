-- Restore tags array column on recipe_versions
ALTER TABLE recipe_versions ADD COLUMN tags CITEXT[] NOT NULL DEFAULT '{}';

-- Migrate data back: populate tags array from junction table
UPDATE recipe_versions rv
SET tags = (
    SELECT COALESCE(array_agg(ut.name ORDER BY ut.name), '{}')
    FROM recipe_version_tags rvt
    JOIN user_tags ut ON ut.id = rvt.tag_id
    WHERE rvt.recipe_version_id = rv.id
);

-- Recreate the GIN index
CREATE INDEX idx_recipe_versions_tags ON recipe_versions USING GIN(tags);

-- Drop junction table and user_tags
DROP TABLE recipe_version_tags;
DROP TABLE user_tags;
