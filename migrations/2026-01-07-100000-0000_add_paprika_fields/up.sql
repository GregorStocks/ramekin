-- Add Paprika-compatible fields for lossless roundtrip import/export
ALTER TABLE recipes ADD COLUMN servings TEXT;
ALTER TABLE recipes ADD COLUMN prep_time TEXT;
ALTER TABLE recipes ADD COLUMN cook_time TEXT;
ALTER TABLE recipes ADD COLUMN total_time TEXT;
ALTER TABLE recipes ADD COLUMN rating INTEGER;
ALTER TABLE recipes ADD COLUMN difficulty TEXT;
ALTER TABLE recipes ADD COLUMN nutritional_info TEXT;
ALTER TABLE recipes ADD COLUMN notes TEXT;
