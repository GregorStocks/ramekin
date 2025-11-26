use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
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

// Ingredient structure for JSONB storage
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Ingredient {
    pub item: String,
    pub amount: Option<String>,
    pub unit: Option<String>,
    pub note: Option<String>,
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
#[diesel(table_name = crate::schema::recipes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct Recipe {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub ingredients: serde_json::Value,
    pub instructions: String,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub photo_ids: Vec<Option<Uuid>>,
    pub tags: Vec<Option<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::recipes)]
pub struct NewRecipe<'a> {
    pub user_id: Uuid,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub ingredients: serde_json::Value,
    pub instructions: &'a str,
    pub source_url: Option<&'a str>,
    pub source_name: Option<&'a str>,
    pub photo_ids: &'a [Option<Uuid>],
    pub tags: &'a [Option<String>],
}
