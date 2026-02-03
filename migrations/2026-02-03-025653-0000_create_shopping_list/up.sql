-- Shopping list: track ingredients to buy
CREATE TABLE shopping_list_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),

    -- Denormalized ingredient data (copied at add time)
    item TEXT NOT NULL,
    amount TEXT,
    note TEXT,

    -- Optional provenance (SET NULL if recipe deleted)
    source_recipe_id UUID REFERENCES recipes(id) ON DELETE SET NULL,
    source_recipe_title TEXT,

    -- State
    is_checked BOOLEAN NOT NULL DEFAULT FALSE,
    sort_order INTEGER NOT NULL DEFAULT 0,

    -- Sync tracking
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    client_id UUID,
    version INTEGER NOT NULL DEFAULT 1
);

-- Index for user's items sorted
CREATE INDEX idx_shopping_list_user ON shopping_list_items(user_id, sort_order);

-- Index for sync queries (changes since timestamp)
CREATE INDEX idx_shopping_list_sync ON shopping_list_items(user_id, updated_at);
