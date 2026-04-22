-- Strip the `raw` key from every ingredient object in recipe_versions.ingredients.
-- The `raw` field (original unparsed ingredient text) was only used for a UI
-- mouse-over that we're removing; users can click through to the source URL
-- for original text.
UPDATE recipe_versions
SET ingredients = COALESCE(
    (
        SELECT jsonb_agg(ing - 'raw' ORDER BY idx)
        FROM jsonb_array_elements(ingredients) WITH ORDINALITY AS t(ing, idx)
    ),
    '[]'::jsonb
)
WHERE ingredients @? '$[*] ? (exists(@.raw))';
