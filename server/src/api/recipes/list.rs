use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::{recipe_versions, recipes};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{Array, Bool, Nullable, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// SQL function declarations for PostgreSQL functions
diesel::define_sql_function! {
    /// PostgreSQL cardinality() function for array length
    fn cardinality(array: Array<Nullable<SqlUuid>>) -> diesel::sql_types::Integer;
}

diesel::define_sql_function! {
    /// PostgreSQL random() function
    fn random() -> diesel::sql_types::Double;
}

/// Sort field for recipe list
#[derive(Debug, Default, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    /// Sort by update time (version created_at)
    #[default]
    UpdatedAt,
    /// Random order (useful for "pick a random recipe")
    Random,
}

/// Sort direction
#[derive(Debug, Default, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Descending (newest/highest first)
    #[default]
    Desc,
    /// Ascending (oldest/lowest first)
    Asc,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListRecipesParams {
    /// Number of items to return (default: 20, max: 1000)
    pub limit: Option<i64>,
    /// Number of items to skip (default: 0)
    pub offset: Option<i64>,
    /// Search query with optional filters. Supports:
    /// - Plain text: searches title and description
    /// - tag:value: filter by tag (can use multiple)
    /// - source:value: filter by source name
    /// - has:photos / no:photos: filter by photo presence
    /// - created:>2024-01-01: created after date
    /// - created:<2024-12-31: created before date
    /// - created:2024-01-01..2024-12-31: created in date range
    ///
    /// Example: "chicken tag:dinner tag:quick has:photos"
    pub q: Option<String>,
    /// Sort field (default: updated_at)
    #[serde(default)]
    pub sort_by: SortBy,
    /// Sort direction (default: desc). Ignored when sort_by=random.
    #[serde(default)]
    pub sort_dir: Direction,
}

/// Parsed search query components
#[derive(Debug, Default)]
struct ParsedQuery {
    text: Vec<String>,
    tags: Vec<String>,
    source: Option<String>,
    has_photos: Option<bool>,
    created_after: Option<NaiveDate>,
    created_before: Option<NaiveDate>,
}

fn parse_query(q: &str) -> ParsedQuery {
    let mut result = ParsedQuery::default();

    // Simple tokenizer: split on whitespace, but respect quotes
    let tokens = tokenize(q);

    for token in tokens {
        if let Some(tag) = token.strip_prefix("tag:") {
            if !tag.is_empty() {
                result.tags.push(tag.to_string());
            }
        } else if let Some(source) = token.strip_prefix("source:") {
            if !source.is_empty() {
                result.source = Some(source.to_string());
            }
        } else if token == "has:photos" || token == "has:photo" {
            result.has_photos = Some(true);
        } else if token == "no:photos" || token == "no:photo" {
            result.has_photos = Some(false);
        } else if let Some(date_expr) = token.strip_prefix("created:") {
            parse_date_filter(date_expr, &mut result);
        } else if !token.is_empty() {
            // Plain text search term
            result.text.push(token.to_string());
        }
    }

    result
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in input.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn parse_date_filter(expr: &str, result: &mut ParsedQuery) {
    // Handle range: 2024-01-01..2024-12-31
    if let Some((start, end)) = expr.split_once("..") {
        if let Ok(date) = NaiveDate::parse_from_str(start, "%Y-%m-%d") {
            result.created_after = Some(date);
        }
        if let Ok(date) = NaiveDate::parse_from_str(end, "%Y-%m-%d") {
            result.created_before = Some(date);
        }
        return;
    }

    // Handle >date (after)
    if let Some(date_str) = expr.strip_prefix('>') {
        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            result.created_after = Some(date);
        }
        return;
    }

    // Handle <date (before)
    if let Some(date_str) = expr.strip_prefix('<') {
        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            result.created_before = Some(date);
        }
        return;
    }

    // Handle exact date (treat as single day range)
    if let Ok(date) = NaiveDate::parse_from_str(expr, "%Y-%m-%d") {
        result.created_after = Some(date);
        result.created_before = Some(date);
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginationMetadata {
    /// Total number of items available
    pub total: i64,
    /// Number of items requested (limit)
    pub limit: i64,
    /// Number of items skipped (offset)
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecipeSummary {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Photo ID of the first photo (thumbnail), if any
    pub thumbnail_photo_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListRecipesResponse {
    pub recipes: Vec<RecipeSummary>,
    pub pagination: PaginationMetadata,
}

/// Row type for the recipe list query result
#[derive(Queryable, Selectable)]
#[diesel(table_name = recipes)]
struct RecipeListRow {
    id: Uuid,
    created_at: DateTime<Utc>,
}

/// Version fields we need for the list
#[derive(Queryable, Selectable)]
#[diesel(table_name = recipe_versions)]
struct VersionListRow {
    title: String,
    description: Option<String>,
    tags: Vec<Option<String>>,
    photo_ids: Vec<Option<Uuid>>,
    #[diesel(column_name = created_at)]
    updated_at: DateTime<Utc>,
}

/// Escape special characters for ILIKE patterns
fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[utoipa::path(
    get,
    path = "/api/recipes",
    tag = "recipes",
    params(ListRecipesParams),
    responses(
        (status = 200, description = "List of user's recipes", body = ListRecipesResponse),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_recipes(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Query(params): Query<ListRecipesParams>,
) -> impl IntoResponse {
    // Validate and set defaults for pagination
    let limit = params.limit.unwrap_or(20).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0).max(0);

    // Parse the query string
    let parsed = params.q.as_deref().map(parse_query).unwrap_or_default();

    let mut conn = get_conn!(pool);

    // Build base query with join
    // We use into_boxed() to allow dynamic filter additions
    let mut query = recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .into_boxed();

    // Text search on title OR description
    if !parsed.text.is_empty() {
        let search_text = parsed.text.join(" ");
        let pattern = format!("%{}%", escape_like_pattern(&search_text));
        query = query.filter(
            recipe_versions::title
                .ilike(pattern.clone())
                .or(recipe_versions::description.ilike(pattern)),
        );
    }

    // Tag filters (AND logic - must have ALL tags)
    // Use raw SQL for CITEXT array containment since Diesel doesn't have direct support
    for tag in &parsed.tags {
        let escaped_tag = tag.replace('\'', "''");
        query = query.filter(sql::<Bool>(&format!(
            "'{}'::citext = ANY(recipe_versions.tags)",
            escaped_tag
        )));
    }

    // Source filter
    if let Some(ref source) = parsed.source {
        let pattern = format!("%{}%", escape_like_pattern(source));
        query = query.filter(recipe_versions::source_name.ilike(pattern));
    }

    // Has photos filter
    if let Some(has_photos) = parsed.has_photos {
        if has_photos {
            query = query.filter(cardinality(recipe_versions::photo_ids).gt(0));
        } else {
            query = query.filter(cardinality(recipe_versions::photo_ids).eq(0));
        }
    }

    // Date range filters (on recipe created_at)
    if let Some(after) = parsed.created_after {
        let after_datetime = after.and_hms_opt(0, 0, 0).unwrap().and_utc();
        query = query.filter(recipes::created_at.ge(after_datetime));
    }
    if let Some(before) = parsed.created_before {
        let before_datetime = before.and_hms_opt(23, 59, 59).unwrap().and_utc();
        query = query.filter(recipes::created_at.le(before_datetime));
    }

    // Clone the query for count (before adding order/limit)
    // We need to rebuild since boxed queries can't be easily cloned
    let mut count_query = recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .into_boxed();

    // Apply same filters to count query
    if !parsed.text.is_empty() {
        let search_text = parsed.text.join(" ");
        let pattern = format!("%{}%", escape_like_pattern(&search_text));
        count_query = count_query.filter(
            recipe_versions::title
                .ilike(pattern.clone())
                .or(recipe_versions::description.ilike(pattern)),
        );
    }

    for tag in &parsed.tags {
        let escaped_tag = tag.replace('\'', "''");
        count_query = count_query.filter(sql::<Bool>(&format!(
            "'{}'::citext = ANY(recipe_versions.tags)",
            escaped_tag
        )));
    }

    if let Some(ref source) = parsed.source {
        let pattern = format!("%{}%", escape_like_pattern(source));
        count_query = count_query.filter(recipe_versions::source_name.ilike(pattern));
    }

    if let Some(has_photos) = parsed.has_photos {
        if has_photos {
            count_query = count_query.filter(cardinality(recipe_versions::photo_ids).gt(0));
        } else {
            count_query = count_query.filter(cardinality(recipe_versions::photo_ids).eq(0));
        }
    }

    if let Some(after) = parsed.created_after {
        let after_datetime = after.and_hms_opt(0, 0, 0).unwrap().and_utc();
        count_query = count_query.filter(recipes::created_at.ge(after_datetime));
    }
    if let Some(before) = parsed.created_before {
        let before_datetime = before.and_hms_opt(23, 59, 59).unwrap().and_utc();
        count_query = count_query.filter(recipes::created_at.le(before_datetime));
    }

    // Get total count
    let total: i64 = match count_query.count().get_result(&mut conn) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to count recipes: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipes".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Add ordering
    let query = match (params.sort_by, params.sort_dir) {
        (SortBy::Random, _) => query.order(random()),
        (SortBy::UpdatedAt, Direction::Desc) => query.order(recipe_versions::created_at.desc()),
        (SortBy::UpdatedAt, Direction::Asc) => query.order(recipe_versions::created_at.asc()),
    };

    // Add pagination and execute
    let results: Vec<(RecipeListRow, VersionListRow)> = match query
        .select((RecipeListRow::as_select(), VersionListRow::as_select()))
        .limit(limit)
        .offset(offset)
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to fetch recipes: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipes".to_string(),
                }),
            )
                .into_response();
        }
    };

    let recipes = results
        .into_iter()
        .map(|(recipe, version)| {
            let thumbnail_photo_id = version.photo_ids.first().and_then(|id| *id);

            RecipeSummary {
                id: recipe.id,
                title: version.title,
                description: version.description,
                tags: version.tags.into_iter().flatten().collect(),
                thumbnail_photo_id,
                created_at: recipe.created_at,
                updated_at: version.updated_at,
            }
        })
        .collect();

    (
        StatusCode::OK,
        Json(ListRecipesResponse {
            recipes,
            pagination: PaginationMetadata {
                total,
                limit,
                offset,
            },
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_query() {
        let parsed = parse_query("");
        assert!(parsed.text.is_empty());
        assert!(parsed.tags.is_empty());
        assert!(parsed.source.is_none());
        assert!(parsed.has_photos.is_none());
    }

    #[test]
    fn test_parse_plain_text() {
        let parsed = parse_query("chicken soup");
        assert_eq!(parsed.text, vec!["chicken", "soup"]);
    }

    #[test]
    fn test_parse_tags() {
        let parsed = parse_query("tag:dinner tag:quick");
        assert_eq!(parsed.tags, vec!["dinner", "quick"]);
    }

    #[test]
    fn test_parse_mixed() {
        let parsed = parse_query("chicken tag:dinner source:NYTimes has:photos");
        assert_eq!(parsed.text, vec!["chicken"]);
        assert_eq!(parsed.tags, vec!["dinner"]);
        assert_eq!(parsed.source, Some("NYTimes".to_string()));
        assert_eq!(parsed.has_photos, Some(true));
    }

    #[test]
    fn test_parse_no_photos() {
        let parsed = parse_query("no:photos");
        assert_eq!(parsed.has_photos, Some(false));
    }

    #[test]
    fn test_parse_date_after() {
        let parsed = parse_query("created:>2024-01-15");
        assert_eq!(
            parsed.created_after,
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
        assert!(parsed.created_before.is_none());
    }

    #[test]
    fn test_parse_date_before() {
        let parsed = parse_query("created:<2024-12-31");
        assert!(parsed.created_after.is_none());
        assert_eq!(
            parsed.created_before,
            Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap())
        );
    }

    #[test]
    fn test_parse_date_range() {
        let parsed = parse_query("created:2024-01-01..2024-06-30");
        assert_eq!(
            parsed.created_after,
            Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
        );
        assert_eq!(
            parsed.created_before,
            Some(NaiveDate::from_ymd_opt(2024, 6, 30).unwrap())
        );
    }

    #[test]
    fn test_parse_exact_date() {
        let parsed = parse_query("created:2024-03-15");
        assert_eq!(
            parsed.created_after,
            Some(NaiveDate::from_ymd_opt(2024, 3, 15).unwrap())
        );
        assert_eq!(
            parsed.created_before,
            Some(NaiveDate::from_ymd_opt(2024, 3, 15).unwrap())
        );
    }

    #[test]
    fn test_parse_quoted_text() {
        let parsed = parse_query("\"green beans\" tag:side");
        assert_eq!(parsed.text, vec!["green beans"]);
        assert_eq!(parsed.tags, vec!["side"]);
    }
}
