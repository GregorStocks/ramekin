-- Large step outputs (fetched pages, mostly) move out of the database into
-- the content-addressed file store beside it (server/src/files.rs). A row
-- whose output is in a file names it by the SHA-256 of its contents and
-- holds JSON null in `output`. '' marks an output kept inline: small ones,
-- and large ones written before this, which the server moves out in short
-- batches at startup (scraping::output_store::move_outputs_out).
ALTER TABLE step_outputs ADD COLUMN output_hash TEXT NOT NULL DEFAULT '';
