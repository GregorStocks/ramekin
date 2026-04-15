DROP INDEX IF EXISTS idx_photos_min_dim;
DROP INDEX IF EXISTS idx_photos_file_size;

ALTER TABLE photos
    DROP COLUMN file_size,
    DROP COLUMN height,
    DROP COLUMN width;
