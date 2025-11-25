use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::recipes;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

pub const PATH: &str = "/api/recipes";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecipeSummary {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListRecipesResponse {
    pub recipes: Vec<RecipeSummary>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = recipes)]
struct RecipeForList {
    id: Uuid,
    title: String,
    description: Option<String>,
    tags: Vec<Option<String>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/recipes",
    tag = "recipes",
    responses(
        (status = 200, description = "List of user's recipes", body = ListRecipesResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_recipes(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> impl IntoResponse {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Database connection failed".to_string(),
                }),
            )
                .into_response()
        }
    };

    let results: Vec<RecipeForList> = match recipes::table
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select(RecipeForList::as_select())
        .order(recipes::updated_at.desc())
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipes".to_string(),
                }),
            )
                .into_response()
        }
    };

    let recipes = results
        .into_iter()
        .map(|r| RecipeSummary {
            id: r.id,
            title: r.title,
            description: r.description,
            tags: r.tags.into_iter().flatten().collect(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    (StatusCode::OK, Json(ListRecipesResponse { recipes })).into_response()
}
