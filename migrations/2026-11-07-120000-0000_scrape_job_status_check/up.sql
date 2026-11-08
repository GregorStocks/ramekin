-- A scrape job's status is one of a fixed set (server/src/scraping/job_status.rs),
-- and some columns go with it: a failed job has an error, a completed job has
-- its recipe, and a job that stopped has no step running. SQLite can only add
-- table CHECKs by rebuilding the table, and step_outputs references this one,
-- so this migration runs outside Diesel's transaction (metadata.toml) to turn
-- foreign keys off around its own.
--
-- No row is deleted. Rows that would break a CHECK are backfilled first:
--   * recipe_id comes from the job's save_recipe output where the row lacks
--     it (jobs saved before the save step recorded it on the row);
--   * an unknown status, or completed with no recipe, becomes failed;
--   * a failed job without an error gets a generic one;
--   * a stopped job's current_step is cleared, after a failed job without a
--     failed_at_step takes it from there (failed jobs keep
--     failed_at_step and current_step_started_at, which retry and the status
--     page read).
PRAGMA foreign_keys = OFF;

BEGIN;

UPDATE scrape_jobs
SET recipe_id = (
    SELECT json_extract(step_outputs.output, '$.recipe_id')
    FROM step_outputs
    WHERE step_outputs.scrape_job_id = scrape_jobs.id
      AND step_outputs.step_name = 'save_recipe'
      AND step_outputs.success = 1
      AND json_valid(step_outputs.output)
      AND json_extract(step_outputs.output, '$.recipe_id') IS NOT NULL
    ORDER BY step_outputs.created_at DESC
    LIMIT 1
)
WHERE recipe_id IS NULL;

UPDATE scrape_jobs
SET status = 'failed',
    error_message = coalesce(error_message, 'Job had unknown status ' || status)
WHERE status NOT IN ('pending', 'scraping', 'parsing', 'normalizing', 'completed', 'failed');

UPDATE scrape_jobs
SET status = 'failed',
    error_message = coalesce(error_message, 'Pipeline ended without creating recipe'),
    failed_at_step = coalesce(failed_at_step, 'save_recipe')
WHERE status = 'completed' AND recipe_id IS NULL;

UPDATE scrape_jobs SET error_message = 'Unknown error'
WHERE status = 'failed' AND error_message IS NULL;

UPDATE scrape_jobs SET failed_at_step = current_step
WHERE status = 'failed' AND failed_at_step IS NULL;

UPDATE scrape_jobs SET current_step = NULL
WHERE status IN ('completed', 'failed') AND current_step IS NOT NULL;

CREATE TABLE scrape_jobs_new (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    url TEXT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'scraping', 'parsing', 'normalizing', 'completed', 'failed')),
    failed_at_step TEXT,
    recipe_id TEXT,
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    current_step TEXT,
    photo_only INTEGER NOT NULL DEFAULT 0,
    current_step_started_at TEXT,
    expected_version_id TEXT REFERENCES recipe_versions (id),
    idempotency_key TEXT,
    normalize_ops TEXT,
    edit_request TEXT,
    CHECK (status <> 'failed' OR error_message IS NOT NULL),
    CHECK (status <> 'completed' OR recipe_id IS NOT NULL),
    CHECK (status NOT IN ('completed', 'failed') OR current_step IS NULL)
);

INSERT INTO scrape_jobs_new (
    id, user_id, url, status, failed_at_step, recipe_id, error_message, retry_count,
    created_at, updated_at, current_step, photo_only, current_step_started_at,
    expected_version_id, idempotency_key, normalize_ops, edit_request
)
SELECT
    id, user_id, url, status, failed_at_step, recipe_id, error_message, retry_count,
    created_at, updated_at, current_step, photo_only, current_step_started_at,
    expected_version_id, idempotency_key, normalize_ops, edit_request
FROM scrape_jobs;

DROP TABLE scrape_jobs;
ALTER TABLE scrape_jobs_new RENAME TO scrape_jobs;

CREATE INDEX idx_scrape_jobs_status ON scrape_jobs (status);
CREATE INDEX idx_scrape_jobs_user_id ON scrape_jobs (user_id);
CREATE UNIQUE INDEX scrape_jobs_user_idempotency_key ON scrape_jobs (user_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX scrape_jobs_recipe_id_idx ON scrape_jobs (recipe_id);

COMMIT;

PRAGMA foreign_keys = ON;
