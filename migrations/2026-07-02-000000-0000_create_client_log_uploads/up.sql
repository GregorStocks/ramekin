CREATE TABLE client_log_uploads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    platform TEXT NOT NULL,
    app_version TEXT,
    os_info TEXT,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_client_log_uploads_user_id ON client_log_uploads(user_id) WHERE deleted_at IS NULL;
