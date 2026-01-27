use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::{Ingredient, NewRecipeVersion, NewUserTag, RecipeVersionTag};
use crate::raw_sql;
use crate::schema::{recipe_version_tags, recipe_versions, recipes, user_tags};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateRecipeRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub ingredients: Option<Vec<Ingredient>>,
    pub instructions: Option<String>,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub photo_ids: Option<Vec<Uuid>>,
    pub tags: Option<Vec<String>>,
    // Paprika-compatible fields
    pub servings: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub rating: Option<i32>,
    pub difficulty: Option<String>,
    pub nutritional_info: Option<String>,
    pub notes: Option<String>,
}

// Type alias for the combined recipe + version + tags query result
#[allow(clippy::type_complexity)]
type CurrentVersionRow = (
    Uuid,              // recipes.id
    String,            // recipe_versions.title
    Option<String>,    // description
    serde_json::Value, // ingredients (JSON)
    String,            // instructions
    Option<String>,    // source_url
    Option<String>,    // source_name
    Vec<Option<Uuid>>, // photo_ids
    Option<String>,    // servings
    Option<String>,    // prep_time
    Option<String>,    // cook_time
    Option<String>,    // total_time
    Option<i32>,       // rating
    Option<String>,    // difficulty
    Option<String>,    // nutritional_info
    Option<String>,    // notes
    Vec<String>,       // tags (from correlated subquery)
);

#[utoipa::path(
    put,
    path = "/api/recipes/{id}",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    request_body = UpdateRecipeRequest,
    responses(
        (status = 200, description = "Recipe updated successfully"),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateRecipeRequest>,
) -> impl IntoResponse {
    if let Some(ref title) = request.title {
        if title.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Title cannot be empty".to_string(),
                }),
            )
                .into_response();
        }
    }

    if let Some(ref instructions) = request.instructions {
        if instructions.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Instructions cannot be empty".to_string(),
                }),
            )
                .into_response();
        }
    }

    let mut conn = get_conn!(pool);

    // Fetch recipe, current version, and tags in a single query
    let current: CurrentVersionRow = match recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::id.eq(id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select((
            recipes::id,
            recipe_versions::title,
            recipe_versions::description,
            recipe_versions::ingredients,
            recipe_versions::instructions,
            recipe_versions::source_url,
            recipe_versions::source_name,
            recipe_versions::photo_ids,
            recipe_versions::servings,
            recipe_versions::prep_time,
            recipe_versions::cook_time,
            recipe_versions::total_time,
            recipe_versions::rating,
            recipe_versions::difficulty,
            recipe_versions::nutritional_info,
            recipe_versions::notes,
            raw_sql::tags_subquery(),
        ))
        .first(&mut conn)
    {
        Ok(r) => r,
        Err(diesel::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Recipe not found".to_string(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe".to_string(),
                }),
            )
                .into_response()
        }
    };

    let (
        recipe_id,
        cur_title,
        cur_description,
        cur_ingredients,
        cur_instructions,
        cur_source_url,
        cur_source_name,
        cur_photo_ids,
        cur_servings,
        cur_prep_time,
        cur_cook_time,
        cur_total_time,
        cur_rating,
        cur_difficulty,
        cur_nutritional_info,
        cur_notes,
        cur_tags,
    ) = current;

    // Merge request with current version
    let new_title = request.title.unwrap_or(cur_title);
    let new_description = request.description.or(cur_description);
    let new_ingredients = match request.ingredients {
        Some(ingredients) => match serde_json::to_value(&ingredients) {
            Ok(v) => v,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Invalid ingredients format".to_string(),
                    }),
                )
                    .into_response()
            }
        },
        None => cur_ingredients,
    };
    let new_instructions = request.instructions.unwrap_or(cur_instructions);
    let new_source_url = request.source_url.or(cur_source_url);
    let new_source_name = request.source_name.or(cur_source_name);
    let new_photo_ids: Vec<Option<Uuid>> = request
        .photo_ids
        .map(|ids| ids.into_iter().map(Some).collect())
        .unwrap_or(cur_photo_ids);
    let new_tags: Vec<String> = request.tags.unwrap_or(cur_tags);
    let new_servings = request.servings.or(cur_servings);
    let new_prep_time = request.prep_time.or(cur_prep_time);
    let new_cook_time = request.cook_time.or(cur_cook_time);
    let new_total_time = request.total_time.or(cur_total_time);
    let new_rating = request.rating.or(cur_rating);
    let new_difficulty = request.difficulty.or(cur_difficulty);
    let new_nutritional_info = request.nutritional_info.or(cur_nutritional_info);
    let new_notes = request.notes.or(cur_notes);

    // Create new version in a transaction
    let result: Result<(), diesel::result::Error> = conn.transaction(|conn| {
        let new_version = NewRecipeVersion {
            recipe_id,
            title: &new_title,
            description: new_description.as_deref(),
            ingredients: new_ingredients,
            instructions: &new_instructions,
            source_url: new_source_url.as_deref(),
            source_name: new_source_name.as_deref(),
            photo_ids: &new_photo_ids,
            servings: new_servings.as_deref(),
            prep_time: new_prep_time.as_deref(),
            cook_time: new_cook_time.as_deref(),
            total_time: new_total_time.as_deref(),
            rating: new_rating,
            difficulty: new_difficulty.as_deref(),
            nutritional_info: new_nutritional_info.as_deref(),
            notes: new_notes.as_deref(),
            version_source: "user",
        };

        let version_id: Uuid = diesel::insert_into(recipe_versions::table)
            .values(&new_version)
            .returning(recipe_versions::id)
            .get_result(conn)?;

        // Update recipe to point to new version
        diesel::update(recipes::table.find(recipe_id))
            .set(recipes::current_version_id.eq(version_id))
            .execute(conn)?;

        // Handle tags: upsert into user_tags and insert into junction table
        for tag_name in &new_tags {
            // Upsert the tag into user_tags
            let tag_id: Uuid = diesel::insert_into(user_tags::table)
                .values(NewUserTag {
                    user_id: user.id,
                    name: tag_name,
                })
                .on_conflict((user_tags::user_id, user_tags::name))
                .do_update()
                .set(user_tags::name.eq(user_tags::name)) // No-op update to return the id
                .returning(user_tags::id)
                .get_result(conn)?;

            // Insert into junction table
            diesel::insert_into(recipe_version_tags::table)
                .values(RecipeVersionTag {
                    recipe_version_id: version_id,
                    tag_id,
                })
                .on_conflict_do_nothing()
                .execute(conn)?;
        }

        Ok(())
    });

    match result {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => {
            tracing::error!("Failed to update recipe: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to update recipe".to_string(),
                }),
            )
                .into_response()
        }
    }
}
