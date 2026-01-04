use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::recipes;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Bool, Text};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Sort field for recipe list
#[derive(Debug, Default, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    /// Sort by update time
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

#[derive(Queryable)]
struct RecipeForList {
    id: Uuid,
    title: String,
    description: Option<String>,
    tags: Vec<Option<String>>,
    photo_ids: Vec<Option<Uuid>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    /// Total count of all matching rows (from window function)
    total_count: i64,
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

    // Pre-compute patterns so they live long enough for the boxed queries
    let text_pattern = if !parsed.text.is_empty() {
        let search_text = parsed.text.join(" ");
        Some(format!(
            "%{}%",
            search_text.replace('%', "\\%").replace('_', "\\_")
        ))
    } else {
        None
    };

    let source_pattern = parsed
        .source
        .as_ref()
        .map(|s| format!("%{}%", s.replace('%', "\\%").replace('_', "\\_")));

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

    // Build base query with filters
    let mut query = recipes::table
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .into_boxed();

    // Apply text search (ILIKE on title OR description)
    if let Some(ref pattern) = text_pattern {
        query = query.filter(
            recipes::title
                .ilike(pattern)
                .or(recipes::description.ilike(pattern)),
        );
    }

    // Apply tag filter (AND logic - must have ALL tags)
    // Cast search term to citext for case-insensitive comparison with citext[] column
    for tag in &parsed.tags {
        query = query.filter(
            sql::<Bool>("")
                .bind::<Text, _>(tag)
                .sql("::citext = ANY(tags)"),
        );
    }

    // Apply source filter (case-insensitive substring)
    if let Some(ref pattern) = source_pattern {
        query = query.filter(recipes::source_name.ilike(pattern));
    }

    // Apply has_photos filter
    if let Some(has_photos) = parsed.has_photos {
        if has_photos {
            query = query.filter(sql::<Bool>("cardinality(photo_ids) > 0"));
        } else {
            query = query.filter(sql::<Bool>("cardinality(photo_ids) = 0"));
        }
    }

    // Apply date range filters
    if let Some(after) = parsed.created_after {
        let after_datetime = after.and_hms_opt(0, 0, 0).unwrap().and_utc();
        query = query.filter(recipes::created_at.ge(after_datetime));
    }
    if let Some(before) = parsed.created_before {
        let before_datetime = before.and_hms_opt(23, 59, 59).unwrap().and_utc();
        query = query.filter(recipes::created_at.le(before_datetime));
    }

    // Apply ordering
    let query = match (params.sort_by, params.sort_dir) {
        (SortBy::Random, _) => query.order(sql::<Text>("RANDOM()")),
        (SortBy::UpdatedAt, Direction::Desc) => query.order(recipes::updated_at.desc()),
        (SortBy::UpdatedAt, Direction::Asc) => query.order(recipes::updated_at.asc()),
    };

    // Get paginated results with total count using window function
    // COUNT(*) OVER() computes the total count across all matching rows
    let results: Vec<RecipeForList> = match query
        .select((
            recipes::id,
            recipes::title,
            recipes::description,
            recipes::tags,
            recipes::photo_ids,
            recipes::created_at,
            recipes::updated_at,
            sql::<BigInt>("COUNT(*) OVER()"),
        ))
        .limit(limit)
        .offset(offset)
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

    // Extract total from first result, or 0 if no results
    let total = results.first().map(|r| r.total_count).unwrap_or(0);

    let recipes = results
        .into_iter()
        .map(|r| {
            let thumbnail_photo_id = r.photo_ids.first().and_then(|id| *id);

            RecipeSummary {
                id: r.id,
                title: r.title,
                description: r.description,
                tags: r.tags.into_iter().flatten().collect(),
                thumbnail_photo_id,
                created_at: r.created_at,
                updated_at: r.updated_at,
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
