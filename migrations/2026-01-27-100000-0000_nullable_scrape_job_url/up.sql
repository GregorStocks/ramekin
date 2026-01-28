-- Make scrape_jobs.url nullable to support imports that don't have a source URL
ALTER TABLE scrape_jobs ALTER COLUMN url DROP NOT NULL;
