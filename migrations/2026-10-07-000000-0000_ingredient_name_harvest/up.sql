-- Names the committed catalog learned (a harvest of these answers, or a
-- classification pass) are marked harvested rather than deleted, keeping
-- their last answer. Each answer also records the candidate keys it was
-- offered, so an "unknown" answer is asked again once the catalog offers
-- different ones.
ALTER TABLE ingredient_name_resolutions
    DROP CONSTRAINT ingredient_name_resolutions_status_check,
    ADD CONSTRAINT ingredient_name_resolutions_status_check
        CHECK (status IN ('pending', 'resolved', 'failed', 'harvested')),
    DROP CONSTRAINT ingredient_name_resolutions_check,
    ADD CONSTRAINT ingredient_name_resolutions_check
        CHECK (status = 'harvested' OR (status = 'resolved') = (disposition IS NOT NULL)),
    ADD COLUMN candidates TEXT[];

DROP INDEX ingredient_name_resolutions_open;
CREATE INDEX ingredient_name_resolutions_open
    ON ingredient_name_resolutions (status)
    WHERE status IN ('pending', 'failed');
