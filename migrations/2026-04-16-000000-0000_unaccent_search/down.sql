-- Revert: drop f_unaccent expression indexes and rebuild plain trigram indexes

DROP INDEX IF EXISTS idx_recipe_versions_title_trgm;
DROP INDEX IF EXISTS idx_recipe_versions_description_trgm;
DROP INDEX IF EXISTS idx_recipe_versions_instructions_trgm;
DROP INDEX IF EXISTS idx_recipe_versions_notes_trgm;
DROP INDEX IF EXISTS idx_recipe_versions_ingredients_text_trgm;

CREATE INDEX idx_recipe_versions_title_trgm
ON recipe_versions USING GIN (title gin_trgm_ops);

CREATE INDEX idx_recipe_versions_description_trgm
ON recipe_versions USING GIN (description gin_trgm_ops);

CREATE INDEX idx_recipe_versions_instructions_trgm
ON recipe_versions USING GIN (instructions gin_trgm_ops);

CREATE INDEX idx_recipe_versions_notes_trgm
ON recipe_versions USING GIN (notes gin_trgm_ops);

CREATE INDEX idx_recipe_versions_ingredients_text_trgm
ON recipe_versions USING GIN ((ingredients::text) gin_trgm_ops);

DROP FUNCTION IF EXISTS f_unaccent(text);

-- Intentionally do NOT drop the unaccent extension: it may have existed
-- before this migration or be used by other objects.
