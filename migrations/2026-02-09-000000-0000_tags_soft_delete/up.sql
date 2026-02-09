-- Add soft-delete support to user_tags (previously hard-deleted, violating data retention policy)
--
-- We keep the existing UNIQUE(user_id, name) constraint so there's always exactly
-- one row per tag name per user. Soft-deleted tags are revived (deleted_at cleared)
-- when the tag is used again via ON CONFLICT DO UPDATE SET deleted_at = NULL.
ALTER TABLE user_tags
    ADD COLUMN deleted_at TIMESTAMPTZ;
