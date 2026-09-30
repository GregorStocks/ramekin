-- Names the committed ingredient catalog doesn't know, resolved by an LLM
-- after a recipe or shopping-list item is saved (catalog step 3). Shared across
-- users: a name's meaning doesn't depend on who wrote it. The row carries its
-- own work state, so a name is resolved once however many saves mention it.
CREATE TABLE ingredient_name_resolutions (
    name TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'resolved', 'failed')),
    disposition TEXT CHECK (disposition IN ('entry', 'not_food', 'unknown')),
    catalog_key TEXT,
    model TEXT,
    error TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Only a resolved name has an answer, and only an entry answer has a key.
    CHECK ((status = 'resolved') = (disposition IS NOT NULL)),
    CHECK (((disposition = 'entry') IS TRUE) = (catalog_key IS NOT NULL)),
    CHECK ((status = 'failed') = (error IS NOT NULL))
);

CREATE INDEX ingredient_name_resolutions_open
    ON ingredient_name_resolutions (status)
    WHERE status <> 'resolved';
