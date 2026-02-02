-- Meal planning: assign recipes to dates and meal types
CREATE TABLE meal_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    recipe_id UUID NOT NULL REFERENCES recipes(id),
    meal_date DATE NOT NULL,
    meal_type VARCHAR(20) NOT NULL CHECK (meal_type IN ('breakfast', 'lunch', 'dinner', 'snack')),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

-- Unique constraint for active rows only (allow re-adding after soft delete)
CREATE UNIQUE INDEX idx_meal_plans_unique_slot_active
    ON meal_plans(user_id, meal_date, meal_type, recipe_id)
    WHERE deleted_at IS NULL;

-- Index for querying by user and date range (the primary access pattern)
CREATE INDEX idx_meal_plans_user_date ON meal_plans(user_id, meal_date) WHERE deleted_at IS NULL;

-- Index for finding all meals containing a specific recipe
CREATE INDEX idx_meal_plans_recipe ON meal_plans(recipe_id) WHERE deleted_at IS NULL;
