-- Convert tags back to text[]
ALTER TABLE recipes ALTER COLUMN tags TYPE text[] USING tags::text[];

-- Note: Not dropping citext extension as other things might use it
