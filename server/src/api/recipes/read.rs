use crate::db::DbConn;
use crate::models::RecipeVersion;
use crate::raw_sql;
use crate::schema::recipes;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde_json::Value as JsonValue;
use uuid::Uuid;

macro_rules! current_recipe_versions_for_user {
    ($user_id:expr) => {
        $crate::schema::recipes::table
            .inner_join(
                $crate::schema::recipe_versions::table.on($crate::schema::recipe_versions::id
                    .nullable()
                    .eq($crate::schema::recipes::current_version_id)),
            )
            .filter($crate::schema::recipes::user_id.eq($user_id))
            .filter($crate::schema::recipes::deleted_at.is_null())
    };
}

pub(crate) use current_recipe_versions_for_user;

macro_rules! recipe_summary_select {
    () => {
        (
            $crate::schema::recipes::id,
            $crate::schema::recipes::created_at,
            $crate::schema::recipe_versions::title,
            $crate::schema::recipe_versions::description,
            $crate::schema::recipe_versions::photo_ids,
            $crate::schema::recipe_versions::rating,
            $crate::schema::recipe_versions::created_at,
            $crate::raw_sql::tags_subquery(),
        )
    };
}

pub(crate) use recipe_summary_select;

macro_rules! counted_recipe_summary_select {
    () => {
        (
            $crate::schema::recipes::id,
            $crate::schema::recipes::created_at,
            $crate::schema::recipe_versions::title,
            $crate::schema::recipe_versions::description,
            $crate::schema::recipe_versions::photo_ids,
            $crate::schema::recipe_versions::rating,
            $crate::schema::recipe_versions::created_at,
            diesel::dsl::count_star().over(),
            $crate::raw_sql::tags_subquery(),
        )
    };
}

pub(crate) use counted_recipe_summary_select;

macro_rules! recipe_relevance_select {
    () => {
        (
            $crate::schema::recipes::id,
            $crate::schema::recipes::created_at,
            $crate::schema::recipe_versions::title,
            $crate::schema::recipe_versions::description,
            $crate::schema::recipe_versions::photo_ids,
            $crate::schema::recipe_versions::rating,
            $crate::schema::recipe_versions::created_at,
            $crate::schema::recipe_versions::ingredients,
            $crate::schema::recipe_versions::instructions,
            $crate::schema::recipe_versions::notes,
            $crate::raw_sql::tags_subquery(),
        )
    };
}

pub(crate) use recipe_relevance_select;

/// Recipe identity plus the current version row.
pub struct RecipeWithVersion {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub version: RecipeVersion,
}

#[derive(Queryable)]
pub struct RecipeSummaryRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub title: String,
    pub description: Option<String>,
    pub photo_ids: Vec<Option<Uuid>>,
    pub rating: Option<i32>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

#[derive(Queryable)]
pub struct CountedRecipeSummaryRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub title: String,
    pub description: Option<String>,
    pub photo_ids: Vec<Option<Uuid>>,
    pub rating: Option<i32>,
    pub updated_at: DateTime<Utc>,
    pub total: i64,
    pub tags: Vec<String>,
}

impl CountedRecipeSummaryRow {
    pub fn into_summary_row(self) -> RecipeSummaryRow {
        RecipeSummaryRow {
            id: self.id,
            created_at: self.created_at,
            title: self.title,
            description: self.description,
            photo_ids: self.photo_ids,
            rating: self.rating,
            updated_at: self.updated_at,
            tags: self.tags,
        }
    }
}

#[derive(Queryable)]
pub struct RecipeRelevanceRow {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub title: String,
    pub description: Option<String>,
    pub photo_ids: Vec<Option<Uuid>>,
    pub rating: Option<i32>,
    pub updated_at: DateTime<Utc>,
    pub ingredients: JsonValue,
    pub instructions: String,
    pub notes: Option<String>,
    pub tags: Vec<String>,
}

impl RecipeRelevanceRow {
    pub fn into_summary_row(self) -> RecipeSummaryRow {
        RecipeSummaryRow {
            id: self.id,
            created_at: self.created_at,
            title: self.title,
            description: self.description,
            photo_ids: self.photo_ids,
            rating: self.rating,
            updated_at: self.updated_at,
            tags: self.tags,
        }
    }
}

pub fn fetch_current_recipe_with_version(
    conn: &mut DbConn,
    user_id: Uuid,
    recipe_id: Uuid,
) -> QueryResult<RecipeWithVersion> {
    let (id, created_at, version): (Uuid, DateTime<Utc>, RecipeVersion) =
        current_recipe_versions_for_user!(user_id)
            .filter(recipes::id.eq(recipe_id))
            .select((recipes::id, recipes::created_at, RecipeVersion::as_select()))
            .first(conn)?;

    Ok(RecipeWithVersion {
        id,
        created_at,
        version,
    })
}

pub fn fetch_current_recipe_with_version_and_tags(
    conn: &mut DbConn,
    user_id: Uuid,
    recipe_id: Uuid,
) -> QueryResult<(RecipeWithVersion, Vec<String>)> {
    let (id, created_at, version, tags): (Uuid, DateTime<Utc>, RecipeVersion, Vec<String>) =
        current_recipe_versions_for_user!(user_id)
            .filter(recipes::id.eq(recipe_id))
            .select((
                recipes::id,
                recipes::created_at,
                RecipeVersion::as_select(),
                raw_sql::tags_subquery(),
            ))
            .first(conn)?;

    Ok((
        RecipeWithVersion {
            id,
            created_at,
            version,
        },
        tags,
    ))
}

pub fn fetch_current_recipes_with_versions(
    conn: &mut DbConn,
    user_id: Uuid,
) -> QueryResult<Vec<RecipeWithVersion>> {
    let rows: Vec<(Uuid, DateTime<Utc>, RecipeVersion)> =
        current_recipe_versions_for_user!(user_id)
            .select((recipes::id, recipes::created_at, RecipeVersion::as_select()))
            .load(conn)?;

    Ok(rows
        .into_iter()
        .map(|(id, created_at, version)| RecipeWithVersion {
            id,
            created_at,
            version,
        })
        .collect())
}
