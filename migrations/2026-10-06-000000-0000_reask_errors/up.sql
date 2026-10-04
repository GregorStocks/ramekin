-- Why the last re-ask of a resolved answer failed. The earlier answer keeps
-- being served; the status page shows the error and Retry asks again.
ALTER TABLE ingredient_name_resolutions
    ADD COLUMN reask_error TEXT,
    ADD CONSTRAINT ingredient_name_resolutions_reask_error_check
        CHECK (reask_error IS NULL OR (status = 'resolved' AND NOT reasked));
ALTER TABLE ingredient_weight_estimates
    ADD COLUMN reask_error TEXT,
    ADD CONSTRAINT ingredient_weight_estimates_reask_error_check
        CHECK (reask_error IS NULL OR (status = 'resolved' AND NOT reasked));
