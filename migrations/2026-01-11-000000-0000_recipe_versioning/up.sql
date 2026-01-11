-- Recipe versioning: normalize recipes table, add recipe_versions

-- Drop old recipes table (CASCADE handles FK from scrape_jobs)
DROP TABLE IF EXISTS recipes CASCADE;

-- Create minimal recipes table (just identity + pointer to current version)
CREATE TABLE recipes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    current_version_id UUID,  -- FK added after recipe_versions exists
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

-- Partial index for active recipes by user (most common query pattern)
CREATE INDEX idx_recipes_user_active ON recipes(user_id) WHERE deleted_at IS NULL;

-- For date range filtering on recipe creation date
CREATE INDEX idx_recipes_created_at ON recipes(created_at);

-- Create recipe_versions table (all content lives here)
CREATE TABLE recipe_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recipe_id UUID NOT NULL REFERENCES recipes(id),

    -- All content fields
    title VARCHAR NOT NULL,
    description TEXT,
    ingredients JSONB NOT NULL,
    instructions TEXT NOT NULL,
    source_url VARCHAR,
    source_name VARCHAR,
    photo_ids UUID[] NOT NULL DEFAULT '{}',
    tags CITEXT[] NOT NULL DEFAULT '{}',
    servings TEXT,
    prep_time TEXT,
    cook_time TEXT,
    total_time TEXT,
    rating INTEGER,
    difficulty TEXT,
    nutritional_info TEXT,
    notes TEXT,

    -- Version metadata
    version_source VARCHAR NOT NULL,  -- 'user', 'scrape', 'bookmarklet', 'enrich', etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add FK from recipes to recipe_versions (deferred to avoid circular reference issues)
ALTER TABLE recipes ADD CONSTRAINT fk_recipes_current_version
    FOREIGN KEY (current_version_id) REFERENCES recipe_versions(id);

-- For listing versions of a recipe
CREATE INDEX idx_recipe_versions_recipe_id ON recipe_versions(recipe_id);

-- GIN index for tag array filtering (required for ANY() queries)
CREATE INDEX idx_recipe_versions_tags ON recipe_versions USING GIN(tags);

-- For text search with ILIKE (required - ILIKE without this is very slow)
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX idx_recipe_versions_title_trgm ON recipe_versions USING GIN(title gin_trgm_ops);
CREATE INDEX idx_recipe_versions_description_trgm ON recipe_versions USING GIN(description gin_trgm_ops);
