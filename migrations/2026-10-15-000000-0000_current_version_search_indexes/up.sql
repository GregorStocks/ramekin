-- Text search only ever matches a recipe's current version, but the trigram
-- indexes covered every historical version. A common word matched thousands
-- of old versions, each rechecked with f_unaccent() over the full text before
-- the join to recipes.current_version_id threw it away. Flag the current
-- version and limit the indexes to it.

ALTER TABLE recipe_versions
ADD COLUMN is_current BOOLEAN NOT NULL DEFAULT false;

UPDATE recipe_versions
SET is_current = true
FROM recipes
WHERE recipes.current_version_id = recipe_versions.id;

-- Maintained from the recipes side so every repoint (the shared version
-- write path, restores, manual fixes) keeps the flag in step without each
-- writer having to remember.
CREATE FUNCTION sync_recipe_version_is_current()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF OLD.current_version_id IS NOT DISTINCT FROM NEW.current_version_id THEN
            RETURN NULL;
        END IF;
        IF OLD.current_version_id IS NOT NULL THEN
            UPDATE recipe_versions SET is_current = false
            WHERE id = OLD.current_version_id;
        END IF;
    END IF;
    IF NEW.current_version_id IS NOT NULL THEN
        UPDATE recipe_versions SET is_current = true
        WHERE id = NEW.current_version_id;
    END IF;
    RETURN NULL;
END
$$;

CREATE TRIGGER recipes_sync_recipe_version_is_current
AFTER INSERT OR UPDATE OF current_version_id ON recipes
FOR EACH ROW
EXECUTE FUNCTION sync_recipe_version_is_current();

DROP INDEX idx_recipe_versions_title_trgm;
DROP INDEX idx_recipe_versions_description_trgm;
DROP INDEX idx_recipe_versions_instructions_trgm;
DROP INDEX idx_recipe_versions_notes_trgm;
DROP INDEX idx_recipe_versions_ingredients_text_trgm;

CREATE INDEX idx_recipe_versions_title_trgm
ON recipe_versions USING GIN (f_unaccent(title) gin_trgm_ops)
WHERE is_current;

CREATE INDEX idx_recipe_versions_description_trgm
ON recipe_versions USING GIN (f_unaccent(description) gin_trgm_ops)
WHERE is_current;

CREATE INDEX idx_recipe_versions_instructions_trgm
ON recipe_versions USING GIN (f_unaccent(instructions) gin_trgm_ops)
WHERE is_current;

CREATE INDEX idx_recipe_versions_notes_trgm
ON recipe_versions USING GIN (f_unaccent(notes) gin_trgm_ops)
WHERE is_current;

CREATE INDEX idx_recipe_versions_ingredients_text_trgm
ON recipe_versions USING GIN (f_unaccent(ingredients::text) gin_trgm_ops)
WHERE is_current;
