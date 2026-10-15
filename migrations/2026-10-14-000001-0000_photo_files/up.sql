-- Photo bytes move out of the database into content-addressed files beside
-- it (server/src/photos/store.rs). Each row names its files by the SHA-256
-- of their contents. '' marks a row whose bytes are still in the legacy
-- BLOB columns: at startup, the server copies them out in short batches,
-- then drops those columns (photos::store::move_blobs_out).
ALTER TABLE photos ADD COLUMN data_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE photos ADD COLUMN thumbnail_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE photo_thumbnails ADD COLUMN data_hash TEXT NOT NULL DEFAULT '';
