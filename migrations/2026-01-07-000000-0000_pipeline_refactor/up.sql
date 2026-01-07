-- Add step_data column for pipeline step outputs
-- Rename parsed_data to step_data (same column, new semantics)
ALTER TABLE scrape_jobs RENAME COLUMN parsed_data TO step_data;

-- Set default value for step_data
ALTER TABLE scrape_jobs ALTER COLUMN step_data SET DEFAULT '{}';

-- Drop url_cache table (we store HTML in step_data now)
DROP TABLE url_cache;
