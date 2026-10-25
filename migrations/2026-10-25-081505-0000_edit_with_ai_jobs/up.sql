-- An Edit with AI job: the user's request, as JSON
-- ({"instruction": "...", "photo_ids": [...]}). Such a job also has
-- normalize_ops (empty) and proposes a version the user accepts.
ALTER TABLE scrape_jobs ADD COLUMN edit_request TEXT;
