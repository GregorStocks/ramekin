ALTER TABLE scrape_jobs
    ADD COLUMN current_step_started_at TIMESTAMPTZ NULL;
