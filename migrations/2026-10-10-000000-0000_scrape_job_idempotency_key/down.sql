DROP INDEX scrape_jobs_user_idempotency_key;
ALTER TABLE scrape_jobs DROP COLUMN idempotency_key;
