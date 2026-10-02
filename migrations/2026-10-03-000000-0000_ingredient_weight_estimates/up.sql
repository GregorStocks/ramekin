-- Weights the committed ingredient catalog lacks, estimated by an LLM (catalog
-- step 3): grams per unit of a catalog food with no density ("cup") or no
-- weight for a counted unit ("bunch", "head", "piece" for a bare count).
-- Queued when a calorie estimate reports the gap, and shared across users: a
-- food's weight doesn't depend on who measured it. The row carries its own
-- work state, so each (food, unit) is estimated once.
CREATE TABLE ingredient_weight_estimates (
    -- The catalog entry's id: a curated name or a USDA description.
    food TEXT NOT NULL,
    unit TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'resolved', 'failed')),
    -- Resolved with no grams: the model said there's no typical weight.
    grams DOUBLE PRECISION CHECK (grams > 0),
    model TEXT,
    error TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (food, unit),
    -- Only a resolved estimate has grams.
    CHECK (status = 'resolved' OR grams IS NULL),
    CHECK ((status = 'failed') = (error IS NOT NULL))
);

CREATE INDEX ingredient_weight_estimates_open
    ON ingredient_weight_estimates (status)
    WHERE status <> 'resolved';
