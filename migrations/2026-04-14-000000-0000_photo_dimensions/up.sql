ALTER TABLE photos
    ADD COLUMN width INTEGER,
    ADD COLUMN height INTEGER,
    ADD COLUMN file_size INTEGER;

UPDATE photos SET file_size = octet_length(data) WHERE file_size IS NULL;

CREATE INDEX idx_photos_file_size ON photos (file_size);
CREATE INDEX idx_photos_min_dim ON photos (LEAST(width, height));
