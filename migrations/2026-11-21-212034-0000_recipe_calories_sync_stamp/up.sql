-- A version's calorie estimate is rewritten in place as ingredient names
-- resolve, and recipe sync now carries it, so a change to it has to reach
-- clients like any other change to the recipe.
CREATE TRIGGER recipe_versions_stamp_calories
AFTER UPDATE OF kcal_per_serving, kcal_per_serving_label ON recipe_versions
WHEN NEW.kcal_per_serving IS NOT OLD.kcal_per_serving
    OR NEW.kcal_per_serving_label IS NOT OLD.kcal_per_serving_label
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE recipe_versions SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;
