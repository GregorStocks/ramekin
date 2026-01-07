-- Recreate url_cache table
CREATE TABLE url_cache (
    url VARCHAR PRIMARY KEY,
    content BYTEA NOT NULL,
    content_type VARCHAR,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Remove default from step_data
ALTER TABLE scrape_jobs ALTER COLUMN step_data DROP DEFAULT;

-- Rename step_data back to parsed_data
ALTER TABLE scrape_jobs RENAME COLUMN step_data TO parsed_data;
