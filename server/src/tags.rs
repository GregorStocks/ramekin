use crate::models::NewUserTag;
use crate::schema::user_tags;
use chrono::{DateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

pub fn upsert_user_tag(conn: &mut PgConnection, user_id: Uuid, name: &str) -> QueryResult<Uuid> {
    if let Some(id) = diesel::insert_into(user_tags::table)
        .values(NewUserTag { user_id, name })
        .on_conflict_do_nothing()
        .returning(user_tags::id)
        .get_result(conn)
        .optional()?
    {
        return Ok(id);
    }

    if let Some(id) = diesel::update(
        user_tags::table
            .filter(user_tags::user_id.eq(user_id))
            .filter(user_tags::name.eq(name))
            .filter(user_tags::deleted_at.is_not_null()),
    )
    .set((
        user_tags::deleted_at.eq(None::<DateTime<Utc>>),
        user_tags::updated_at.eq(Utc::now()),
    ))
    .returning(user_tags::id)
    .get_result(conn)
    .optional()?
    {
        return Ok(id);
    }

    user_tags::table
        .filter(user_tags::user_id.eq(user_id))
        .filter(user_tags::name.eq(name))
        .select(user_tags::id)
        .first(conn)
}
