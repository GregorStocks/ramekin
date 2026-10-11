ALTER TABLE ingredient_name_resolutions
    DROP CONSTRAINT ingredient_name_resolutions_status_check,
    ADD CONSTRAINT ingredient_name_resolutions_status_check
        CHECK (status IN ('pending', 'resolved', 'failed', 'harvested')),
    DROP CONSTRAINT ingredient_name_resolutions_check,
    ADD CONSTRAINT ingredient_name_resolutions_check
        CHECK (status = 'harvested' OR (status = 'resolved') = (disposition IS NOT NULL));
