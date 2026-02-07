use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::users)]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub password_hash: &'a str,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::sessions)]
pub struct NewSession<'a> {
    pub user_id: Uuid,
    pub token_hash: &'a str,
    pub expires_at: DateTime<Utc>,
}

/// A single measurement (amount + unit pair)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Measurement {
    pub amount: Option<String>,
    pub unit: Option<String>,
}

/// Ingredient structure for JSONB storage
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Ingredient {
    /// The ingredient name (e.g., "butter", "all-purpose flour")
    pub item: String,
    /// Measurements - first is primary, rest are alternatives (e.g., "1 stick" then "113g")
    pub measurements: Vec<Measurement>,
    /// Preparation notes (e.g., "chopped", "softened", "optional")
    pub note: Option<String>,
    /// Original unparsed text for debugging
    pub raw: Option<String>,
    /// Section name for grouping (e.g., "For the sauce", "For the dough")
    pub section: Option<String>,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::photos)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct Photo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content_type: String,
    pub data: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub thumbnail: Vec<u8>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::photos)]
pub struct NewPhoto<'a> {
    pub user_id: Uuid,
    pub content_type: &'a str,
    pub data: &'a [u8],
    pub thumbnail: &'a [u8],
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::photo_thumbnails)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct PhotoThumbnail {
    pub id: Uuid,
    pub photo_id: Uuid,
    pub size: i32,
    pub data: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::photo_thumbnails)]
pub struct NewPhotoThumbnail<'a> {
    pub photo_id: Uuid,
    pub size: i32,
    pub data: &'a [u8],
}

// Recipe is now minimal - just identity + pointer to current version
#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::recipes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct Recipe {
    pub id: Uuid,
    pub user_id: Uuid,
    pub current_version_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::recipes)]
pub struct NewRecipe {
    pub user_id: Uuid,
}

// RecipeVersion contains all recipe content (normalized from recipes table)
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::recipe_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct RecipeVersion {
    pub id: Uuid,
    pub recipe_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub ingredients: serde_json::Value,
    pub instructions: String,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub photo_ids: Vec<Option<Uuid>>,
    pub servings: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub rating: Option<i32>,
    pub difficulty: Option<String>,
    pub nutritional_info: Option<String>,
    pub notes: Option<String>,
    pub version_source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::recipe_versions)]
pub struct NewRecipeVersion<'a> {
    pub recipe_id: Uuid,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub ingredients: serde_json::Value,
    pub instructions: &'a str,
    pub source_url: Option<&'a str>,
    pub source_name: Option<&'a str>,
    pub photo_ids: &'a [Option<Uuid>],
    pub servings: Option<&'a str>,
    pub prep_time: Option<&'a str>,
    pub cook_time: Option<&'a str>,
    pub total_time: Option<&'a str>,
    pub rating: Option<i32>,
    pub difficulty: Option<&'a str>,
    pub nutritional_info: Option<&'a str>,
    pub notes: Option<&'a str>,
    pub version_source: &'a str,
}

// Scrape job for async URL scraping
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::scrape_jobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct ScrapeJob {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: Option<String>,
    pub status: String,
    pub failed_at_step: Option<String>,
    pub recipe_id: Option<Uuid>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub current_step: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::scrape_jobs)]
pub struct NewScrapeJob<'a> {
    pub user_id: Uuid,
    pub url: Option<&'a str>,
}

// Step output for pipeline step results (append-only log)
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::step_outputs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct StepOutput {
    pub id: Uuid,
    pub scrape_job_id: Uuid,
    pub step_name: String,
    pub build_id: String,
    pub output: JsonValue,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::step_outputs)]
pub struct NewStepOutput {
    pub scrape_job_id: Uuid,
    pub step_name: String,
    pub build_id: String,
    pub output: JsonValue,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::user_tags)]
pub struct NewUserTag<'a> {
    pub user_id: Uuid,
    pub name: &'a str,
}

// Junction table for recipe version <-> tag associations
#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::recipe_version_tags)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RecipeVersionTag {
    pub recipe_version_id: Uuid,
    pub tag_id: Uuid,
}

// Meal planning: assign recipes to dates and meal types
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::meal_plans)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct MealPlan {
    pub id: Uuid,
    pub user_id: Uuid,
    pub recipe_id: Uuid,
    pub meal_date: chrono::NaiveDate,
    pub meal_type: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::meal_plans)]
pub struct NewMealPlan<'a> {
    pub user_id: Uuid,
    pub recipe_id: Uuid,
    pub meal_date: chrono::NaiveDate,
    pub meal_type: &'a str,
    pub notes: Option<&'a str>,
}

// Shopping list: track ingredients to buy (offline-capable)
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::shopping_list_items)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct ShoppingListItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub item: String,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub source_recipe_id: Option<Uuid>,
    pub source_recipe_title: Option<String>,
    pub is_checked: bool,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub client_id: Option<Uuid>,
    pub version: i32,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::shopping_list_items)]
pub struct NewShoppingListItem<'a> {
    pub user_id: Uuid,
    pub item: &'a str,
    pub amount: Option<&'a str>,
    pub note: Option<&'a str>,
    pub source_recipe_id: Option<Uuid>,
    pub source_recipe_title: Option<&'a str>,
    pub is_checked: bool,
    pub sort_order: i32,
    pub client_id: Option<Uuid>,
}
