-- A version's per-serving calorie estimate, for sorting and filtering the
-- recipe list (ramekin_core::nutrition::ServingFigure). Null when the
-- estimate gives no per-serving figure. Derived data: the server stores it
-- when a version is written, recomputes it as ingredient names resolve, and
-- fills and corrects every current version at startup, so this leaves
-- existing rows null.
ALTER TABLE recipe_versions ADD COLUMN kcal_per_serving REAL;
ALTER TABLE recipe_versions ADD COLUMN kcal_per_serving_label TEXT
    CHECK ((kcal_per_serving IS NULL) = (kcal_per_serving_label IS NULL));
