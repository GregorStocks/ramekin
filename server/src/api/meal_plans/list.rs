use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::{meal_plans, recipe_versions, recipes};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MealType {
    Breakfast,
    Lunch,
    Dinner,
    Snack,
}

impl MealType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MealType::Breakfast => "breakfast",
            MealType::Lunch => "lunch",
            MealType::Dinner => "dinner",
            MealType::Snack => "snack",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "breakfast" => Some(MealType::Breakfast),
            "lunch" => Some(MealType::Lunch),
            "dinner" => Some(MealType::Dinner),
            "snack" => Some(MealType::Snack),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListMealPlansParams {
    /// Start date (inclusive), format: YYYY-MM-DD. Defaults to today.
    pub start_date: Option<NaiveDate>,
    /// End date (inclusive), format: YYYY-MM-DD. Defaults to start_date + 6 days (one week).
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MealPlanItem {
    pub id: Uuid,
    pub recipe_id: Uuid,
    pub recipe_title: String,
    pub thumbnail_photo_id: Option<Uuid>,
    pub meal_date: NaiveDate,
    pub meal_type: MealType,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MealPlanListResponse {
    pub meal_plans: Vec<MealPlanItem>,
}

// Type alias for query result row
type MealPlanRow = (
    Uuid,              // meal_plan.id
    Uuid,              // recipe_id
    NaiveDate,         // meal_date
    String,            // meal_type
    Option<String>,    // notes
    String,            // recipe_version.title
    Vec<Option<Uuid>>, // recipe_version.photo_ids
);

#[utoipa::path(
    get,
    path = "/api/meal-plans",
    tag = "meal_plans",
    params(ListMealPlansParams),
    responses(
        (status = 200, description = "List of meal plans for date range", body = MealPlanListResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_meal_plans(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Query(params): Query<ListMealPlansParams>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let today = chrono::Utc::now().date_naive();
    let start_date = params.start_date.unwrap_or(today);
    let end_date = params
        .end_date
        .unwrap_or(start_date + chrono::Duration::days(6));

    // Query meal_plans joined with recipes and recipe_versions to get title and photo
    let rows: Vec<MealPlanRow> = match meal_plans::table
        .inner_join(recipes::table)
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(meal_plans::user_id.eq(user.id))
        .filter(meal_plans::deleted_at.is_null())
        .filter(recipes::deleted_at.is_null())
        .filter(meal_plans::meal_date.ge(start_date))
        .filter(meal_plans::meal_date.le(end_date))
        .select((
            meal_plans::id,
            meal_plans::recipe_id,
            meal_plans::meal_date,
            meal_plans::meal_type,
            meal_plans::notes,
            recipe_versions::title,
            recipe_versions::photo_ids,
        ))
        .order((meal_plans::meal_date.asc(), meal_plans::meal_type.asc()))
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to fetch meal plans: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch meal plans".to_string(),
                }),
            )
                .into_response();
        }
    };

    let meal_plans = rows
        .into_iter()
        .filter_map(
            |(id, recipe_id, meal_date, meal_type_str, notes, title, photo_ids)| {
                let meal_type = MealType::from_str(&meal_type_str)?;
                let thumbnail_photo_id = photo_ids.into_iter().flatten().next();
                Some(MealPlanItem {
                    id,
                    recipe_id,
                    recipe_title: title,
                    thumbnail_photo_id,
                    meal_date,
                    meal_type,
                    notes,
                })
            },
        )
        .collect();

    (StatusCode::OK, Json(MealPlanListResponse { meal_plans })).into_response()
}
