DROP INDEX IF EXISTS idx_recipes_user_tags;
DROP INDEX IF EXISTS idx_recipes_user_description_search;
DROP INDEX IF EXISTS idx_recipes_user_title_search;

-- Restore original tags index
CREATE INDEX idx_recipes_tags ON recipes USING GIN(tags);

-- Note: Not dropping extensions as other things might use them
