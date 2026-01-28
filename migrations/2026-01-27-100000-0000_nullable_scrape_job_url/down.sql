-- Restore NOT NULL constraint on scrape_jobs.url
-- Note: This will fail if any rows have NULL url values
ALTER TABLE scrape_jobs ALTER COLUMN url SET NOT NULL;
