ALTER TABLE photos
    ADD COLUMN width INTEGER,
    ADD COLUMN height INTEGER,
    ADD COLUMN file_size INTEGER;

-- width/height/file_size are left NULL for pre-existing photos. A background
-- task on server startup (src/photos/backfill.rs) decodes each pending photo
-- once and fills in all three. We use `file_size IS NULL` as the "needs
-- backfill" flag so undecodable photos can record file_size without being
-- forced to invent fake dimensions.

CREATE INDEX idx_photos_file_size ON photos (file_size);
CREATE INDEX idx_photos_min_dim ON photos (LEAST(width, height));
