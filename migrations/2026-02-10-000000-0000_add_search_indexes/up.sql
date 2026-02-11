-- Add trigram GIN indexes for full-text ILIKE search on instructions, notes, and ingredients
-- Matches existing pattern for title and description indexes

CREATE INDEX idx_recipe_versions_instructions_trgm
ON recipe_versions USING GIN(instructions gin_trgm_ops);

CREATE INDEX idx_recipe_versions_notes_trgm
ON recipe_versions USING GIN(notes gin_trgm_ops);

-- Expression index on the text cast of the JSONB column
CREATE INDEX idx_recipe_versions_ingredients_text_trgm
ON recipe_versions USING GIN((ingredients::text) gin_trgm_ops);
