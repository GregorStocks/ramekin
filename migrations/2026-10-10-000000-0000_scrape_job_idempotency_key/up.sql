-- Client-chosen key that makes resubmitting an import return the original
-- job instead of creating a duplicate recipe. Scoped per user.
ALTER TABLE scrape_jobs ADD COLUMN idempotency_key TEXT;

CREATE UNIQUE INDEX scrape_jobs_user_idempotency_key
    ON scrape_jobs (user_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
