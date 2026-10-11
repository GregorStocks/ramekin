-- One queue, no re-asks: answers are kept until the catalog stops accepting
-- them, and names the catalog learned are simply not consulted. Harvested
-- rows go back to resolved (keeping their answer) or, with no answer, to
-- pending. Re-ask flags and errors are cleared; the columns stay, unread.
UPDATE ingredient_name_resolutions
SET status = 'resolved'
WHERE status = 'harvested' AND disposition IS NOT NULL;

UPDATE ingredient_name_resolutions
SET status = 'pending'
WHERE status = 'harvested';

UPDATE ingredient_name_resolutions
SET reasked = false, reask_error = NULL
WHERE reasked OR reask_error IS NOT NULL;

UPDATE ingredient_weight_estimates
SET reasked = false, reask_error = NULL
WHERE reasked OR reask_error IS NOT NULL;

ALTER TABLE ingredient_name_resolutions
    DROP CONSTRAINT ingredient_name_resolutions_check,
    ADD CONSTRAINT ingredient_name_resolutions_check
        CHECK ((status = 'resolved') = (disposition IS NOT NULL)),
    DROP CONSTRAINT ingredient_name_resolutions_status_check,
    ADD CONSTRAINT ingredient_name_resolutions_status_check
        CHECK (status IN ('pending', 'resolved', 'failed'));
