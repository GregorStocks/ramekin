-- Add Paprika-compatible fields for lossless roundtrip import/export
ALTER TABLE recipes ADD COLUMN servings VARCHAR;
ALTER TABLE recipes ADD COLUMN prep_time VARCHAR;
ALTER TABLE recipes ADD COLUMN cook_time VARCHAR;
ALTER TABLE recipes ADD COLUMN total_time VARCHAR;
ALTER TABLE recipes ADD COLUMN rating INTEGER;
ALTER TABLE recipes ADD COLUMN difficulty VARCHAR;
ALTER TABLE recipes ADD COLUMN nutritional_info TEXT;
ALTER TABLE recipes ADD COLUMN notes TEXT;
