-- A resolved answer from a model other than the current ingredient model is
-- asked again (flagged at startup), while the old answer keeps being served
-- until the new one replaces it.
ALTER TABLE ingredient_name_resolutions
    ADD COLUMN reasked BOOLEAN NOT NULL DEFAULT false,
    ADD CONSTRAINT ingredient_name_resolutions_reasked_check
        CHECK (NOT reasked OR status = 'resolved');
ALTER TABLE ingredient_weight_estimates
    ADD COLUMN reasked BOOLEAN NOT NULL DEFAULT false,
    ADD CONSTRAINT ingredient_weight_estimates_reasked_check
        CHECK (NOT reasked OR status = 'resolved');
