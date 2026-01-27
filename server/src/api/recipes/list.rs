use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::raw_sql;
use crate::schema::{recipe_version_tags, recipe_versions, recipes, user_tags};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Array, Nullable, Uuid as SqlUuid};
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

/// Escape special characters for ILIKE patterns
fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

// Type alias for our query result row (tags fetched separately now)
type RecipeRow = (
    Uuid,              // recipe id
    Uuid,              // version id (needed to fetch tags)
    DateTime<Utc>,     // recipe created_at
    String,            // version title
    Option<String>,    // version description
    Vec<Option<Uuid>>, // version photo_ids
    DateTime<Utc>,     // version created_at (updated_at)
    i64,               // total count from window function
);

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
    // Use EXISTS subquery for each tag
    for tag in &parsed.tags {
        let tag_subquery = recipe_version_tags::table
            .inner_join(user_tags::table)
            .filter(recipe_version_tags::recipe_version_id.eq(recipe_versions::id))
            .filter(user_tags::name.eq(tag))
            .select(recipe_version_tags::recipe_version_id);
        query = query.filter(diesel::dsl::exists(tag_subquery));
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
        if let Some(time) = after.and_hms_opt(0, 0, 0) {
            let after_datetime = time.and_utc();
            query = query.filter(recipes::created_at.ge(after_datetime));
        }
    }
    if let Some(before) = parsed.created_before {
        if let Some(time) = before.and_hms_opt(23, 59, 59) {
            let before_datetime = time.and_utc();
            query = query.filter(recipes::created_at.le(before_datetime));
        }
    }

    // Add ordering
    let query = match (params.sort_by, params.sort_dir) {
        (SortBy::Random, _) => query.order(random()),
        (SortBy::UpdatedAt, Direction::Desc) => query.order(recipe_versions::created_at.desc()),
        (SortBy::UpdatedAt, Direction::Asc) => query.order(recipe_versions::created_at.asc()),
    };

    // Select columns including COUNT(*) OVER() for total in single query
    // Tags are fetched separately via junction table
    let results: Vec<RecipeRow> = match query
        .select((
            recipes::id,
            recipe_versions::id,
            recipes::created_at,
            recipe_versions::title,
            recipe_versions::description,
            recipe_versions::photo_ids,
            recipe_versions::created_at,
            raw_sql::count_over(),
        ))
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

    // Extract total from first row, or 0 if no results
    let total = results.first().map(|r| r.7).unwrap_or(0);

    // Fetch tags for all versions in one query
    let version_ids: Vec<Uuid> = results.iter().map(|r| r.1).collect();
    let tags_by_version: std::collections::HashMap<Uuid, Vec<String>> = if version_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let tag_rows: Vec<(Uuid, String)> = recipe_version_tags::table
            .inner_join(user_tags::table)
            .filter(recipe_version_tags::recipe_version_id.eq_any(&version_ids))
            .select((recipe_version_tags::recipe_version_id, user_tags::name))
            .load(&mut conn)
            .unwrap_or_default();

        let mut map: std::collections::HashMap<Uuid, Vec<String>> =
            std::collections::HashMap::new();
        for (version_id, tag_name) in tag_rows {
            map.entry(version_id).or_default().push(tag_name);
        }
        map
    };

    let recipes = results
        .into_iter()
        .map(
            |(id, version_id, created_at, title, description, photo_ids, updated_at, _)| {
                let thumbnail_photo_id = photo_ids.first().and_then(|id| *id);
                let tags = tags_by_version
                    .get(&version_id)
                    .cloned()
                    .unwrap_or_default();

                RecipeSummary {
                    id,
                    title,
                    description,
                    tags,
                    thumbnail_photo_id,
                    created_at,
                    updated_at,
                }
            },
        )
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
