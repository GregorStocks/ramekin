-- Add url column back and make data nullable
ALTER TABLE photos ALTER COLUMN data DROP NOT NULL;
ALTER TABLE photos ADD COLUMN url VARCHAR;
ALTER TABLE photos ADD CONSTRAINT photos_check CHECK ((data IS NOT NULL) != (url IS NOT NULL));
