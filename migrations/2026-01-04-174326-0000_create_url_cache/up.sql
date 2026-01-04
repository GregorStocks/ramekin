CREATE TABLE url_cache (
    url VARCHAR PRIMARY KEY,
    content BYTEA NOT NULL,
    content_type VARCHAR,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
