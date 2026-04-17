-- Accent-insensitive search: enable unaccent extension and rebuild trigram
-- indexes as expression indexes on an immutable unaccent wrapper.

CREATE EXTENSION IF NOT EXISTS unaccent;

-- unaccent() is STABLE (dictionary can change), so it can't be used directly
-- in an expression index. Wrap it in an IMMUTABLE SQL function.
CREATE OR REPLACE FUNCTION f_unaccent(text)
RETURNS text
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
STRICT
AS $$ SELECT public.unaccent('public.unaccent', $1) $$;

-- Drop old trigram indexes and rebuild as expression indexes on f_unaccent(col)
DROP INDEX IF EXISTS idx_recipe_versions_title_trgm;
DROP INDEX IF EXISTS idx_recipe_versions_description_trgm;
DROP INDEX IF EXISTS idx_recipe_versions_instructions_trgm;
DROP INDEX IF EXISTS idx_recipe_versions_notes_trgm;
DROP INDEX IF EXISTS idx_recipe_versions_ingredients_text_trgm;

CREATE INDEX idx_recipe_versions_title_trgm
ON recipe_versions USING GIN (f_unaccent(title) gin_trgm_ops);

CREATE INDEX idx_recipe_versions_description_trgm
ON recipe_versions USING GIN (f_unaccent(description) gin_trgm_ops);

CREATE INDEX idx_recipe_versions_instructions_trgm
ON recipe_versions USING GIN (f_unaccent(instructions) gin_trgm_ops);

CREATE INDEX idx_recipe_versions_notes_trgm
ON recipe_versions USING GIN (f_unaccent(notes) gin_trgm_ops);

CREATE INDEX idx_recipe_versions_ingredients_text_trgm
ON recipe_versions USING GIN (f_unaccent(ingredients::text) gin_trgm_ops);
