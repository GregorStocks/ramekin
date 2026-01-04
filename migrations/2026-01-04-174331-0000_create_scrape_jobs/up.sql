CREATE TABLE scrape_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    url VARCHAR NOT NULL,
    status VARCHAR NOT NULL DEFAULT 'pending',
    failed_at_step VARCHAR,
    parsed_data JSONB,
    recipe_id UUID REFERENCES recipes(id),
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_scrape_jobs_user_id ON scrape_jobs(user_id);
CREATE INDEX idx_scrape_jobs_status ON scrape_jobs(status);
