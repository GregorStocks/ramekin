-- Data the calorie redesign (doc/calorie-estimates.md, "Architecture") no
-- longer reads, dropped with the owner's approval: the re-ask flags and
-- errors (cleared by single_name_queue), the candidate lists that drove
-- re-asks, the runtime weights queue (weights now live in the committed
-- catalog), and the categorizer version that drove startup touches
-- (store_shopping_category). The CHECK constraints on these columns go with
-- them.
ALTER TABLE ingredient_name_resolutions
    DROP COLUMN reasked,
    DROP COLUMN reask_error,
    DROP COLUMN candidates;

DROP TABLE ingredient_weight_estimates;

ALTER TABLE shopping_list_items DROP COLUMN categorizer_version;
