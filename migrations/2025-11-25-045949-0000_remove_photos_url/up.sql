-- Remove the url column and make data required
ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_check;
ALTER TABLE photos DROP COLUMN url;
ALTER TABLE photos ALTER COLUMN data SET NOT NULL;
