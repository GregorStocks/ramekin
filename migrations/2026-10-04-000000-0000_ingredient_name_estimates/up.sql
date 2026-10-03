-- A name no catalog entry matches can now be resolved as an "estimate": a real
-- food with the model's own calories per 100 g, and cup and piece weights when
-- it gave them. Estimates label the line.
ALTER TABLE ingredient_name_resolutions
    DROP CONSTRAINT ingredient_name_resolutions_disposition_check,
    ADD CONSTRAINT ingredient_name_resolutions_disposition_check
        CHECK (disposition IN ('entry', 'estimate', 'not_food', 'unknown')),
    ADD COLUMN kcal_per_100g DOUBLE PRECISION CHECK (kcal_per_100g >= 0),
    ADD COLUMN grams_per_cup DOUBLE PRECISION CHECK (grams_per_cup > 0),
    ADD COLUMN grams_per_piece DOUBLE PRECISION CHECK (grams_per_piece > 0),
    -- Only an estimate has numbers, and it always has calories.
    ADD CONSTRAINT ingredient_name_resolutions_estimate_check
        CHECK (((disposition = 'estimate') IS TRUE) = (kcal_per_100g IS NOT NULL)),
    ADD CONSTRAINT ingredient_name_resolutions_estimate_weights_check
        CHECK ((disposition = 'estimate') IS TRUE
            OR (grams_per_cup IS NULL AND grams_per_piece IS NULL));

-- Names the model couldn't match before may be foods it can now estimate, so
-- ask about them again.
UPDATE ingredient_name_resolutions
SET status = 'pending', disposition = NULL, updated_at = now()
WHERE disposition = 'unknown';
