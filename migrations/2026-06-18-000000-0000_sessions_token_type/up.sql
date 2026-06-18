-- Distinguish long-lived, scoped bookmarklet tokens from normal login sessions.
ALTER TABLE sessions ADD COLUMN token_type TEXT NOT NULL DEFAULT 'session';
