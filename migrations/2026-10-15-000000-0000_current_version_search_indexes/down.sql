DROP INDEX idx_recipe_versions_title_trgm;
DROP INDEX idx_recipe_versions_description_trgm;
DROP INDEX idx_recipe_versions_instructions_trgm;
DROP INDEX idx_recipe_versions_notes_trgm;
DROP INDEX idx_recipe_versions_ingredients_text_trgm;

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

DROP TRIGGER recipes_sync_recipe_version_is_current ON recipes;
DROP FUNCTION sync_recipe_version_is_current();

ALTER TABLE recipe_versions DROP COLUMN is_current;
