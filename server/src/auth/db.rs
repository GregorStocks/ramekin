use crate::db::DbPool;
use crate::models::{NewSession, User};
use crate::schema::{sessions, users};
use chrono::{Duration, Utc};
use diesel::prelude::*;

use super::crypto::{generate_token, hash_token};

/// Fixed token for the test user "t" - allows persistent sessions across database resets
pub const DEV_TEST_TOKEN: &str = "tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt";

/// `token_type` for normal login sessions (30-day sliding expiry).
pub const TOKEN_TYPE_SESSION: &str = "session";
/// `token_type` for long-lived, scope-restricted bookmarklet tokens.
pub const TOKEN_TYPE_BOOKMARKLET: &str = "bookmarklet";

pub fn create_session_with_token(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
    fixed_token: Option<&str>,
) -> Result<String, diesel::result::Error> {
    let token = fixed_token
        .map(|t| t.to_string())
        .unwrap_or_else(generate_token);
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::days(30);

    let new_session = NewSession {
        user_id,
        token_hash: &token_hash,
        expires_at,
        token_type: TOKEN_TYPE_SESSION,
    };

    if fixed_token.is_some() {
        diesel::insert_into(sessions::table)
            .values(&new_session)
            .on_conflict(sessions::token_hash)
            .do_update()
            .set(sessions::expires_at.eq(expires_at))
            .execute(conn)?;
    } else {
        diesel::insert_into(sessions::table)
            .values(&new_session)
            .execute(conn)?;
    }

    Ok(token)
}

/// Mint a new long-lived, scope-restricted bookmarklet token for `user_id`.
///
/// These never meaningfully expire (far-future `expires_at`, well beyond the
/// sliding-expiry window so [`get_user_from_token`] never shortens them) and
/// are restricted by `require_auth` to the capture endpoints. Arbitrarily many
/// may exist per user; minting a fresh one does not invalidate older ones, so
/// previously-saved bookmarklets keep working.
pub fn create_bookmarklet_token(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
) -> Result<String, diesel::result::Error> {
    let token = generate_token();
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::days(365 * 100);

    let new_session = NewSession {
        user_id,
        token_hash: &token_hash,
        expires_at,
        token_type: TOKEN_TYPE_BOOKMARKLET,
    };
    diesel::insert_into(sessions::table)
        .values(&new_session)
        .execute(conn)?;

    Ok(token)
}

/// Resolve a bearer token to its user and `token_type`.
///
/// Returns `None` for unknown, expired, or soft-deleted-user tokens. The
/// `token_type` lets callers (`require_auth`) apply per-type scope rules.
pub async fn get_user_from_token(pool: &DbPool, token: &str) -> Option<(User, String)> {
    let mut conn = pool.get().ok()?;
    let token_hash = hash_token(token);
    let now = Utc::now();

    let (user, token_type) = sessions::table
        .inner_join(users::table)
        .filter(sessions::token_hash.eq(&token_hash))
        .filter(sessions::expires_at.gt(now))
        .filter(users::deleted_at.is_null())
        .select((User::as_select(), sessions::token_type))
        .first::<(User, String)>(&mut conn)
        .ok()?;

    // Sliding expiry: bump the session's expires_at when it's aged at least a
    // day past its last touch. The filter caps writes at one per session per
    // day while keeping active sessions from hitting the 30-day wall.
    // Bookmarklet tokens sit far in the future, so this never shortens them.
    let new_expiry = now + Duration::days(30);
    let _ = diesel::update(sessions::table)
        .filter(sessions::token_hash.eq(&token_hash))
        .filter(sessions::expires_at.lt(new_expiry - Duration::days(1)))
        .set(sessions::expires_at.eq(new_expiry))
        .execute(&mut conn);

    Some((user, token_type))
}
