UPDATE ingredient_name_resolutions
SET disposition = 'unknown', kcal_per_100g = NULL, grams_per_cup = NULL, grams_per_piece = NULL
WHERE disposition = 'estimate';
ALTER TABLE ingredient_name_resolutions
    DROP CONSTRAINT ingredient_name_resolutions_estimate_weights_check,
    DROP CONSTRAINT ingredient_name_resolutions_estimate_check,
    DROP COLUMN grams_per_piece,
    DROP COLUMN grams_per_cup,
    DROP COLUMN kcal_per_100g,
    DROP CONSTRAINT ingredient_name_resolutions_disposition_check,
    ADD CONSTRAINT ingredient_name_resolutions_disposition_check
        CHECK (disposition IN ('entry', 'not_food', 'unknown'));
