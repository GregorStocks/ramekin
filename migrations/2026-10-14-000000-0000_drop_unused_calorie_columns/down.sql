-- Restores the structure only; the dropped data is gone.
ALTER TABLE shopping_list_items ADD COLUMN categorizer_version TEXT;

CREATE TABLE ingredient_weight_estimates (
    food TEXT NOT NULL,
    unit TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'resolved', 'failed')),
    grams DOUBLE PRECISION CHECK (grams > 0),
    model TEXT,
    error TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reasked BOOLEAN NOT NULL DEFAULT false,
    reask_error TEXT,
    PRIMARY KEY (food, unit),
    CHECK (status = 'resolved' OR grams IS NULL),
    CHECK ((status = 'failed') = (error IS NOT NULL)),
    CONSTRAINT ingredient_weight_estimates_reasked_check
        CHECK (NOT reasked OR status = 'resolved'),
    CONSTRAINT ingredient_weight_estimates_reask_error_check
        CHECK (reask_error IS NULL OR (status = 'resolved' AND NOT reasked))
);

CREATE INDEX ingredient_weight_estimates_open
    ON ingredient_weight_estimates (status)
    WHERE status <> 'resolved';

ALTER TABLE ingredient_name_resolutions
    ADD COLUMN reasked BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN reask_error TEXT,
    ADD COLUMN candidates TEXT[],
    ADD CONSTRAINT ingredient_name_resolutions_reasked_check
        CHECK (NOT reasked OR status = 'resolved'),
    ADD CONSTRAINT ingredient_name_resolutions_reask_error_check
        CHECK (reask_error IS NULL OR (status = 'resolved' AND NOT reasked));
