-- Recreate url_cache table
CREATE TABLE url_cache (
    url VARCHAR PRIMARY KEY,
    content BYTEA NOT NULL,
    content_type VARCHAR,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Re-add parsed_data column
ALTER TABLE scrape_jobs ADD COLUMN parsed_data JSONB;

-- Drop current_step column
ALTER TABLE scrape_jobs DROP COLUMN current_step;

-- Drop step_outputs table
DROP TABLE step_outputs;
