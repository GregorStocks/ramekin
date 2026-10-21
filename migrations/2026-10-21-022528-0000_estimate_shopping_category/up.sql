-- An estimated food has no catalog entry to take a shopping aisle from, so
-- the model gives one with the estimate. Estimates stored before this have
-- none; the server puts them back in the queue at startup.
ALTER TABLE ingredient_name_resolutions ADD COLUMN shopping_category TEXT
    CHECK (disposition IS 'estimate' OR shopping_category IS NULL);
