-- Add thumbnail column for photo previews
ALTER TABLE photos ADD COLUMN thumbnail BYTEA NOT NULL;
