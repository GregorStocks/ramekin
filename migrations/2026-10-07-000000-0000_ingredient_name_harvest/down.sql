DROP INDEX ingredient_name_resolutions_open;
CREATE INDEX ingredient_name_resolutions_open
    ON ingredient_name_resolutions (status)
    WHERE status <> 'resolved';

UPDATE ingredient_name_resolutions
SET status = 'resolved'
WHERE status = 'harvested' AND disposition IS NOT NULL;

UPDATE ingredient_name_resolutions
SET status = 'pending'
WHERE status = 'harvested';

ALTER TABLE ingredient_name_resolutions
    DROP COLUMN candidates,
    DROP CONSTRAINT ingredient_name_resolutions_check,
    ADD CONSTRAINT ingredient_name_resolutions_check
        CHECK ((status = 'resolved') = (disposition IS NOT NULL)),
    DROP CONSTRAINT ingredient_name_resolutions_status_check,
    ADD CONSTRAINT ingredient_name_resolutions_status_check
        CHECK (status IN ('pending', 'resolved', 'failed'));
