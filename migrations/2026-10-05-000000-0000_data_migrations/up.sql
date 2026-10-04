-- Data migrations written in Rust (server/src/data_migrations.rs) that have
-- run, so each runs once. They run at server startup, after the SQL
-- migrations.
CREATE TABLE data_migrations (
    name TEXT PRIMARY KEY,
    run_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
