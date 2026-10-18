-- Meal plans join the sync feeds (doc/sync-design.md): every insert and
-- update takes a sync_clock stamp, like the other synced tables.
ALTER TABLE meal_plans ADD COLUMN change_seq INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_meal_plans_user_change_seq ON meal_plans (user_id, change_seq);

CREATE TRIGGER meal_plans_stamp_insert AFTER INSERT ON meal_plans
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE meal_plans SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;

CREATE TRIGGER meal_plans_stamp_update AFTER UPDATE ON meal_plans
WHEN NEW.change_seq IS OLD.change_seq
BEGIN
    UPDATE sync_clock SET value = value + 1 WHERE id = 1;
    UPDATE meal_plans SET change_seq = (SELECT value FROM sync_clock WHERE id = 1)
    WHERE id = NEW.id;
END;

-- Shopping items are now created under the client's own id, and writes are
-- per-field last-writer-wins, so `client_id` and `version` are no longer
-- read or written. The columns stay: live data is never dropped.
