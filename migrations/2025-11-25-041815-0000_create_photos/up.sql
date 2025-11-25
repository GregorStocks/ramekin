CREATE TABLE photos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    content_type VARCHAR NOT NULL,
    data BYTEA,
    url VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    CHECK ((data IS NOT NULL) != (url IS NOT NULL))
);

CREATE INDEX idx_photos_user_id ON photos(user_id);
