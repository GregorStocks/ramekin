-- Revert recipe versioning (WARNING: data loss - versions not preserved)

-- Drop new tables
DROP TABLE IF EXISTS recipe_versions CASCADE;
DROP TABLE IF EXISTS recipes CASCADE;

-- Recreate old recipes table with full schema
CREATE TABLE recipes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
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
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_recipes_user_id ON recipes(user_id);
CREATE INDEX idx_recipes_tags ON recipes USING GIN(tags);
