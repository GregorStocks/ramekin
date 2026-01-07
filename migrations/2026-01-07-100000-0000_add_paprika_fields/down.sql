-- Remove Paprika-compatible fields
ALTER TABLE recipes DROP COLUMN servings;
ALTER TABLE recipes DROP COLUMN prep_time;
ALTER TABLE recipes DROP COLUMN cook_time;
ALTER TABLE recipes DROP COLUMN total_time;
ALTER TABLE recipes DROP COLUMN rating;
ALTER TABLE recipes DROP COLUMN difficulty;
ALTER TABLE recipes DROP COLUMN nutritional_info;
ALTER TABLE recipes DROP COLUMN notes;
