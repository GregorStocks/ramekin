use crate::db::DbPool;
use crate::models::{NewSession, User};
use crate::schema::{sessions, users};
use chrono::{Duration, Utc};
use diesel::prelude::*;

use super::crypto::{generate_token, hash_token};

/// Fixed token for the test user "t" - allows persistent sessions across database resets
pub const DEV_TEST_TOKEN: &str = "tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt";

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
    };

    diesel::insert_into(sessions::table)
        .values(&new_session)
        .execute(conn)?;

    Ok(token)
}

pub async fn get_user_from_token(pool: &DbPool, token: &str) -> Option<User> {
    let mut conn = pool.get().ok()?;
    let token_hash = hash_token(token);

    sessions::table
        .inner_join(users::table)
        .filter(sessions::token_hash.eq(&token_hash))
        .filter(sessions::expires_at.gt(Utc::now()))
        .filter(users::deleted_at.is_null())
        .select(User::as_select())
        .first(&mut conn)
        .ok()
}
