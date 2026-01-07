-- Create step_outputs table for pipeline step results (append-only log)
CREATE TABLE step_outputs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scrape_job_id UUID NOT NULL REFERENCES scrape_jobs(id),
    step_name VARCHAR NOT NULL,
    build_id VARCHAR NOT NULL,
    output JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for looking up steps by job
CREATE INDEX step_outputs_scrape_job_id_idx ON step_outputs(scrape_job_id);

-- Add current_step column to track pipeline progress
ALTER TABLE scrape_jobs ADD COLUMN current_step VARCHAR;

-- Drop the old parsed_data column (no longer needed)
ALTER TABLE scrape_jobs DROP COLUMN parsed_data;

-- Drop url_cache table (HTML stored in step_outputs now)
DROP TABLE url_cache;
