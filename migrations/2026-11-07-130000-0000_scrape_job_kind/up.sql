-- A scrape job stores its kind (server/src/scraping/job_kind.rs) instead of
-- leaving every reader to infer it from which nullable columns are set.
-- SQLite can only add a NOT NULL column without a default, or table CHECKs,
-- by rebuilding the table, and step_outputs references this one, so this
-- migration runs outside Diesel's transaction (metadata.toml) to turn
-- foreign keys off around its own.
--
-- Existing rows are backfilled from the columns the server used to decode,
-- in the order it decoded them (edit beat normalize beat photo-only), plus
-- the outputs a job was created with: a capture starts with a stored page,
-- an import with a stored extraction, a text import with its text. No row
-- is deleted. Legacy photo-only rescrapes that never recorded an expected
-- version are backfilled with the recipe's version at job creation.
PRAGMA foreign_keys = OFF;

BEGIN;

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
    kind TEXT NOT NULL CHECK (kind IN (
        'url_scrape', 'capture', 'import', 'text_import', 'photo_import',
        'photo_rescrape', 'rescrape', 'normalize', 'edit'
    )),
    CHECK (status <> 'failed' OR error_message IS NOT NULL),
    CHECK (status <> 'completed' OR recipe_id IS NOT NULL),
    CHECK (status NOT IN ('completed', 'failed') OR current_step IS NULL),
    -- What each kind needs to run.
    CHECK (kind NOT IN ('url_scrape', 'capture', 'photo_rescrape', 'rescrape') OR url IS NOT NULL),
    CHECK (kind NOT IN ('photo_rescrape', 'rescrape', 'normalize', 'edit')
        OR (recipe_id IS NOT NULL AND expected_version_id IS NOT NULL)),
    CHECK (kind IN ('photo_rescrape', 'rescrape', 'normalize', 'edit') OR expected_version_id IS NULL),
    CHECK (photo_only = (kind = 'photo_rescrape')),
    CHECK ((kind = 'normalize') = (normalize_ops IS NOT NULL AND edit_request IS NULL)
        OR kind = 'edit'),
    CHECK ((kind = 'edit') = (edit_request IS NOT NULL))
);

INSERT INTO scrape_jobs_new (
    id, user_id, url, status, failed_at_step, recipe_id, error_message, retry_count,
    created_at, updated_at, current_step, photo_only, current_step_started_at,
    expected_version_id, idempotency_key, normalize_ops, edit_request, kind
)
SELECT
    id, user_id, url, status, failed_at_step, recipe_id, error_message, retry_count,
    created_at, updated_at, current_step, photo_only, current_step_started_at,
    -- Photo-only rescrapes from before jobs recorded the version they
    -- expected (a 2026-04-15 batch) get the recipe's version as of the
    -- job's creation, which is what the job ran against.
    CASE
        WHEN expected_version_id IS NULL AND photo_only AND edit_request IS NULL
            AND normalize_ops IS NULL
        THEN (
            SELECT v.id FROM recipe_versions v
            WHERE v.recipe_id = scrape_jobs.recipe_id
              AND v.created_at <= scrape_jobs.created_at
            ORDER BY v.created_at DESC, v.change_seq DESC
            LIMIT 1
        )
        ELSE expected_version_id
    END,
    idempotency_key, normalize_ops, edit_request,
    CASE
        WHEN edit_request IS NOT NULL THEN 'edit'
        WHEN normalize_ops IS NOT NULL THEN 'normalize'
        WHEN photo_only THEN 'photo_rescrape'
        WHEN expected_version_id IS NOT NULL THEN 'rescrape'
        WHEN url IS NULL AND EXISTS (
            SELECT 1 FROM step_outputs o
            WHERE o.scrape_job_id = scrape_jobs.id AND o.step_name = 'source_text'
        ) THEN 'text_import'
        WHEN idempotency_key IS NOT NULL THEN 'import'
        WHEN url IS NULL AND EXISTS (
            SELECT 1 FROM step_outputs o
            WHERE o.scrape_job_id = scrape_jobs.id
              AND o.step_name = 'extract_recipe'
              AND o.duration_ms IS NULL
              AND json_valid(o.output)
              AND json_extract(o.output, '$.method_used') IS NOT 'photo_upload'
        ) THEN 'import'
        WHEN url IS NULL THEN 'photo_import'
        -- A page stored when the job was created (no run, so no duration).
        WHEN EXISTS (
            SELECT 1 FROM step_outputs o
            WHERE o.scrape_job_id = scrape_jobs.id
              AND o.step_name = 'fetch_html'
              AND o.duration_ms IS NULL
              AND o.success = 1
        ) THEN 'capture'
        -- An import with a source URL starts with its extraction stored and
        -- never fetches the page.
        WHEN EXISTS (
            SELECT 1 FROM step_outputs o
            WHERE o.scrape_job_id = scrape_jobs.id
              AND o.step_name = 'extract_recipe'
              AND o.duration_ms IS NULL
        ) AND NOT EXISTS (
            SELECT 1 FROM step_outputs o
            WHERE o.scrape_job_id = scrape_jobs.id AND o.step_name = 'fetch_html'
        ) THEN 'import'
        ELSE 'url_scrape'
    END
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
