use super::Window;
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::schema::user_tags;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct SyncTagsParams {
    /// Cursor returned by the previous sync. Absent means a full sync.
    pub cursor: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncTag {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncTagsResponse {
    /// Live tags created, renamed or revived at or after `cursor`; every live
    /// tag on a full resync.
    pub changes: Vec<SyncTag>,
    /// Tag IDs deleted at or after `cursor`. Empty on a full resync.
    pub deleted: Vec<Uuid>,
    /// Pass as `cursor` on the next sync.
    pub cursor: i64,
    /// True when `changes` holds every live tag: drop cached tags it omits.
    pub full_resync: bool,
}

#[utoipa::path(
    get,
    path = "/api/sync/tags",
    tag = "sync",
    params(SyncTagsParams),
    responses(
        (status = 200, description = "Tag changes for the local cache", body = SyncTagsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn sync_tags(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Query(params): Query<SyncTagsParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let response = run_db(&pool, move |conn| {
        conn.transaction(|conn| {
            let window = Window::read(conn, params.cursor)?;

            let mut changed = user_tags::table
                .filter(user_tags::user_id.eq(user_id))
                .filter(user_tags::deleted_at.is_null())
                .into_boxed();
            if let Some(since) = window.since {
                changed = changed.filter(user_tags::change_seq.ge(since));
            }
            let changes = changed
                .select((user_tags::id, user_tags::name))
                .order(user_tags::name_key.asc())
                .load::<(Uuid, String)>(conn)?
                .into_iter()
                .map(|(id, name)| SyncTag { id, name })
                .collect();

            let deleted = match window.since {
                Some(since) => user_tags::table
                    .filter(user_tags::user_id.eq(user_id))
                    .filter(user_tags::deleted_at.is_not_null())
                    .filter(user_tags::change_seq.ge(since))
                    .select(user_tags::id)
                    .load(conn)?,
                None => Vec::new(),
            };

            Ok::<_, diesel::result::Error>(SyncTagsResponse {
                changes,
                deleted,
                cursor: window.cursor,
                full_resync: window.full_resync(),
            })
        })
        .map_err(|e| {
            tracing::error!("Failed to sync tags: {}", e);
            ApiError::internal("Failed to sync tags")
        })
    })
    .await?;

    Ok((StatusCode::OK, Json(response)))
}
