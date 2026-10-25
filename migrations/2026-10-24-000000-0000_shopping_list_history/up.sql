-- The shopping list's history: one entry per sync or undo that changed the
-- list, holding each changed item's state before and after, so the change
-- can be shown and undone. Ids are time-ordered (v7), newest last.
CREATE TABLE shopping_list_history (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')),
    -- The entry this one undid, and the entry that undid this one.
    undoes_id TEXT REFERENCES shopping_list_history (id),
    undone_by_id TEXT REFERENCES shopping_list_history (id)
);
CREATE INDEX idx_shopping_list_history_user ON shopping_list_history (user_id, id);

-- `before` and `after` are the item's fields as JSON, or NULL when the item
-- wasn't live (before a create or restore, after a delete).
CREATE TABLE shopping_list_history_events (
    history_id TEXT NOT NULL REFERENCES shopping_list_history (id),
    position INTEGER NOT NULL,
    item_id TEXT NOT NULL REFERENCES shopping_list_items (id),
    op TEXT NOT NULL CHECK (op IN ('create', 'update', 'delete', 'restore')),
    before TEXT,
    after TEXT,
    PRIMARY KEY (history_id, position),
    CHECK (before IS NOT NULL OR after IS NOT NULL)
);
