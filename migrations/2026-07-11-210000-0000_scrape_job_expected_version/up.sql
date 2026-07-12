ALTER TABLE scrape_jobs
ADD COLUMN expected_version_id UUID REFERENCES recipe_versions(id);
