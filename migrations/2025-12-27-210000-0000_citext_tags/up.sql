-- Enable citext extension for case-insensitive text
CREATE EXTENSION IF NOT EXISTS citext;

-- Convert tags column to citext[] for case-insensitive array containment
ALTER TABLE recipes ALTER COLUMN tags TYPE citext[] USING tags::citext[];
