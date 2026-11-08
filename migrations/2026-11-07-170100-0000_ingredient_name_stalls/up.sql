-- Why a pending name is still waiting, when the provider (or the database)
-- failed the worker's last pass, and when the worker will try again at the
-- latest. A provider failure isn't the name's fault, so it doesn't count
-- toward `attempts`; before this it was only in the server's log and memory,
-- and the queue just looked quiet. Cleared when the name leaves pending.
ALTER TABLE ingredient_name_resolutions ADD COLUMN last_error TEXT
    CHECK (last_error IS NULL OR status = 'pending');
ALTER TABLE ingredient_name_resolutions ADD COLUMN next_attempt_at TEXT
    CHECK ((next_attempt_at IS NULL) = (last_error IS NULL));
