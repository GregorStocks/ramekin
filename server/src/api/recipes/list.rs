use super::read::{
    counted_recipe_summary_select, current_recipe_versions_for_user, recipe_relevance_select,
    recipe_summary_select, CountedRecipeSummaryRow, RecipeRelevanceRow, RecipeSummaryRow,
};
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::{self, DbPool};
use crate::ids::Uuid;
use crate::models::Ingredient;
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
use ramekin_core::created_date_filter::{day_end_utc_exclusive, day_start_utc};
use ramekin_core::search::normalize_for_search;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use utoipa::{IntoParams, ToSchema};

/// Sort field for recipe list
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    /// Sort by relevance to the search query's text terms (title matches
    /// rank above tag/description/ingredient/instruction matches). Ignores
    /// sort_dir. With no text terms, equivalent to updated_at desc.
    Relevance,
    /// Sort by update time (version created_at)
    UpdatedAt,
    /// Sort by rating (1-5 stars)
    Rating,
    /// Sort by title (alphabetical)
    Title,
    /// Sort by creation time (recipe created_at)
    CreatedAt,
    /// Sort by estimated calories per serving (a partial estimate's lower
    /// bound). Recipes without a per-serving estimate come last either way.
    Calories,
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
    /// - created:>2024-01-01: created on or after date
    /// - created:<2024-12-31: created on or before date
    /// - created:2024-01-01..2024-12-31: created in date range
    /// - calories:300..600, calories:300.., calories:..600: estimated
    ///   calories per serving, inclusive (calories:>300 and calories:<600 are
    ///   the same as the open ranges). A partial estimate counts by its lower
    ///   bound; recipes without a per-serving estimate never match.
    ///
    /// Date filters name inclusive UTC calendar days.
    ///
    /// Example: "chicken tag:dinner tag:quick has:photos"
    pub q: Option<String>,
    /// Sort field. Defaults to relevance when the query has text terms,
    /// otherwise updated_at.
    // value_type avoids utoipa's `oneOf [null, $ref]` encoding of Option,
    // which openapi-generator's Rust client renders as invalid code. The
    // param is already non-required; absent is the only "null" we need.
    #[param(value_type = SortBy, required = false)]
    pub sort_by: Option<SortBy>,
    /// Sort direction (default: desc). Ignored when sort_by is random or
    /// relevance.
    #[serde(default)]
    pub sort_dir: Direction,
}

/// Numeric threshold used for photo size/dimension filters.
#[derive(Debug, Clone, Copy)]
struct NumericThreshold {
    op: &'static str, // "<" or ">"
    value: i32,
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
    photo_size: Option<NumericThreshold>,
    photo_dim: Option<NumericThreshold>,
    calories_min: Option<f64>,
    calories_max: Option<f64>,
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
        } else if let Some(expr) = token.strip_prefix("photo_size:") {
            result.photo_size = parse_numeric_threshold(expr);
        } else if let Some(expr) = token.strip_prefix("photo_dim:") {
            result.photo_dim = parse_numeric_threshold(expr);
        } else if let Some(expr) = token.strip_prefix("calories:") {
            parse_calorie_filter(expr, &mut result);
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

fn parse_numeric_threshold(expr: &str) -> Option<NumericThreshold> {
    if let Some(rest) = expr.strip_prefix('<') {
        rest.parse::<i32>()
            .ok()
            .map(|value| NumericThreshold { op: "<", value })
    } else if let Some(rest) = expr.strip_prefix('>') {
        rest.parse::<i32>()
            .ok()
            .map(|value| NumericThreshold { op: ">", value })
    } else {
        None
    }
}

/// `300..600`, `300..`, `..600`, `>300` or `<600`, all inclusive. A bound is
/// plain digits with an optional decimal part; anything else is ignored, like
/// a bad date. Pinned for both clients by shared-test-vectors/calorie-filter.json.
fn parse_calorie_filter(expr: &str, result: &mut ParsedQuery) {
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    let bound = |text: &str| {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, "0"));
        (digits(whole) && digits(fraction))
            .then(|| text.parse::<f64>().ok())
            .flatten()
    };
    if let Some((min, max)) = expr.split_once("..") {
        result.calories_min = bound(min);
        result.calories_max = bound(max);
    } else if let Some(min) = expr.strip_prefix('>') {
        result.calories_min = bound(min);
    } else if let Some(max) = expr.strip_prefix('<') {
        result.calories_max = bound(max);
    }
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
    /// Rating from 1-5, if set
    pub rating: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Estimated calories per serving, for display: "~520 kcal",
    /// "~380–560 kcal", or "~520+ kcal" when some ingredients couldn't be
    /// counted. Null when there's no per-serving estimate.
    pub calories_per_serving: Option<String>,
}

impl RecipeSummary {
    pub(crate) fn from_row(row: RecipeSummaryRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            tags: row.tags,
            thumbnail_photo_id: row.photo_ids.first().and_then(|id| *id),
            rating: row.rating,
            created_at: row.created_at,
            updated_at: row.updated_at,
            calories_per_serving: row.calories_per_serving,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListRecipesResponse {
    pub recipes: Vec<RecipeSummary>,
    pub pagination: PaginationMetadata,
}

/// The FTS5 query matching `token` as a substring of any indexed field, or
/// None when the normalized token is too short for the trigram index (under
/// three characters) and has to be matched by a scan instead.
/// The cookbook's filter for one search word: it appears in the recipe's
/// title, description, instructions, notes or ingredients, ignoring case and
/// accents. Words are ANDed by filtering once per word.
pub(crate) fn search_word_filter<'a, QS: 'a>(
    token: &str,
) -> Box<
    dyn diesel::BoxableExpression<QS, diesel::sqlite::Sqlite, SqlType = diesel::sql_types::Bool>
        + 'a,
> {
    match search_index_query(token) {
        Some(fts_query) => Box::new(raw_sql::recipe_search_match(fts_query)),
        // Too short for the trigram index: match the index's stored text
        // with LIKE instead.
        None => raw_sql::recipe_search_like(literal_search_pattern(token)),
    }
}

fn search_index_query(token: &str) -> Option<String> {
    let normalized = normalize_for_search(token);
    if normalized.chars().count() < 3 {
        return None;
    }
    // One quoted phrase: a trigram phrase matches as a substring, and
    // quoting keeps FTS5 from reading operators or column filters in it.
    Some(format!("\"{}\"", normalized.replace('"', "\"\"")))
}

/// Normalize before escaping: normalization can introduce LIKE metacharacters.
fn literal_search_pattern(s: &str) -> String {
    let escaped = normalize_for_search(s)
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
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
) -> Result<impl IntoResponse, ApiError> {
    // Validate and set defaults for pagination
    let limit = params.limit.unwrap_or(20).clamp(1, 1000);
    let offset = params.offset.unwrap_or(0).max(0);

    // Parse the query string
    let parsed = params.q.as_deref().map(parse_query).unwrap_or_default();

    // Default sort: relevance when there are text terms to rank against,
    // recency otherwise.
    let sort_by = params.sort_by.unwrap_or(if parsed.text.is_empty() {
        SortBy::UpdatedAt
    } else {
        SortBy::Relevance
    });
    let sort_dir = params.sort_dir;
    let user_id = user.id;

    let response = run_db(&pool, move |conn| {
        list_recipes_blocking(conn, user_id, &parsed, sort_by, sort_dir, limit, offset)
    })
    .await?;

    Ok((StatusCode::OK, Json(response)))
}

type CurrentRecipeVersionsQuery = diesel::dsl::Filter<
    diesel::dsl::Filter<
        diesel::dsl::InnerJoinOn<
            recipes::table,
            recipe_versions::table,
            diesel::dsl::Eq<
                diesel::dsl::Nullable<recipe_versions::id>,
                recipes::current_version_id,
            >,
        >,
        diesel::dsl::Eq<recipes::user_id, Uuid>,
    >,
    diesel::dsl::IsNull<recipes::deleted_at>,
>;

type FilteredRecipesQuery<'a> =
    diesel::dsl::IntoBoxed<'a, CurrentRecipeVersionsQuery, diesel::sqlite::Sqlite>;

/// The user's current recipe versions narrowed by every filter in `parsed`.
fn filtered_recipes_query(user_id: Uuid, parsed: &ParsedQuery) -> FilteredRecipesQuery<'_> {
    // We use into_boxed() to allow dynamic filter additions.
    let mut query = current_recipe_versions_for_user!(user_id).into_boxed();

    // Text search: each word must appear somewhere across all fields (AND
    // between words, OR between fields). Matches are case- AND
    // accent-insensitive ("creme brulee" finds "Crème Brûlée").
    for token in &parsed.text {
        query = query.filter(search_word_filter(token));
    }

    // Tag filters (AND logic - must have ALL tags)
    // Use EXISTS subquery for each tag
    for tag in &parsed.tags {
        let tag_subquery = recipe_version_tags::table
            .inner_join(user_tags::table)
            .filter(recipe_version_tags::recipe_version_id.eq(recipe_versions::id))
            .filter(user_tags::name_key.eq(db::case_key(tag)))
            .filter(user_tags::deleted_at.is_null())
            .select(recipe_version_tags::recipe_version_id);
        query = query.filter(diesel::dsl::exists(tag_subquery));
    }

    // Source filter (accent- and case-insensitive)
    if let Some(ref source) = parsed.source {
        let pattern = literal_search_pattern(source);
        query = query.filter(
            db::normalize_for_search(recipe_versions::source_name)
                .like(pattern)
                .escape('\\'),
        );
    }

    // Has photos filter
    if let Some(has_photos) = parsed.has_photos {
        if has_photos {
            query = query.filter(raw_sql::json_array_length(recipe_versions::photo_ids).gt(0));
        } else {
            query = query.filter(raw_sql::json_array_length(recipe_versions::photo_ids).eq(0));
        }
    }

    // Photo file size filter (bytes)
    if let Some(threshold) = parsed.photo_size {
        query = query.filter(raw_sql::photo_file_size_filter(
            threshold.op,
            threshold.value,
        ));
    }

    // Photo dimension filter (pixels, compares smaller of width/height)
    if let Some(threshold) = parsed.photo_dim {
        query = query.filter(raw_sql::photo_min_dim_filter(threshold.op, threshold.value));
    }

    // Date range filters (on recipe created_at). The filter dates name
    // inclusive UTC calendar days; the shared bounds keep the iOS cache
    // filter's day semantics and this query in agreement.
    if let Some(after) = parsed.created_after {
        query = query.filter(recipes::created_at.ge(day_start_utc(after)));
    }
    if let Some(before) = parsed.created_before {
        query = query.filter(recipes::created_at.lt(day_end_utc_exclusive(before)));
    }

    // Calorie filters: a recipe with no stored estimate is null, which
    // neither bound matches.
    if let Some(min) = parsed.calories_min {
        query = query.filter(recipe_versions::kcal_per_serving.ge(min));
    }
    if let Some(max) = parsed.calories_max {
        query = query.filter(recipe_versions::kcal_per_serving.le(max));
    }

    query
}

/// Blocking DB and ranking work for `list_recipes`; runs on the blocking
/// thread pool via `run_db`.
fn list_recipes_blocking(
    conn: &mut diesel::SqliteConnection,
    user_id: Uuid,
    parsed: &ParsedQuery,
    sort_by: SortBy,
    sort_dir: Direction,
    limit: i64,
    offset: i64,
) -> Result<ListRecipesResponse, ApiError> {
    // Build the filtered query on demand so an empty page can rerun the same
    // filters as a count query without changing the single-query populated-page
    // path.
    let build_query = || filtered_recipes_query(user_id, parsed);

    let query = build_query();

    // Relevance can't be a SQL ORDER BY: the scorer is a pure Rust function
    // (ramekin_core::search) so a client can mirror it for local search
    // later. Load every matching row and rank in memory — search result
    // sets are one user's matching recipes, which is small.
    if matches!(sort_by, SortBy::Relevance) {
        let rows: Vec<RecipeRelevanceRow> = query
            .select(recipe_relevance_select!())
            .load(conn)
            .map_err(|e| {
                tracing::error!("Failed to fetch recipes for relevance sort: {:?}", e);
                ApiError::internal("Failed to fetch recipes")
            })?;

        let mut scored: Vec<(u32, RecipeSummary)> = Vec::with_capacity(rows.len());
        for row in rows {
            let ingredients: Vec<Ingredient> = serde_json::from_value(row.ingredients.clone())
                .map_err(|e| {
                    tracing::error!(
                        recipe_id = %row.id,
                        error = %e,
                        "stored ingredients JSON failed to deserialize"
                    );
                    ApiError::internal("Recipe ingredients are corrupt")
                })?;
            // One text per ingredient covering everything the search filter
            // can match in the stored JSON (measurements included), so tokens like
            // "cups" score instead of matching silently.
            let ingredient_texts: Vec<String> = ingredients
                .into_iter()
                .map(|i| {
                    let mut parts: Vec<String> = Vec::new();
                    for m in i.measurements {
                        parts.extend(m.amount);
                        parts.extend(m.unit);
                    }
                    parts.push(i.item);
                    parts.extend(i.note);
                    parts.extend(i.section);
                    parts.join(" ")
                })
                .collect();

            let score = ramekin_core::search::relevance_score(
                &parsed.text,
                &ramekin_core::search::SearchDoc {
                    title: &row.title,
                    description: row.description.as_deref(),
                    tags: &row.tags,
                    ingredients: &ingredient_texts,
                    instructions: &row.instructions,
                    notes: row.notes.as_deref(),
                },
            );

            scored.push((score, RecipeSummary::from_row(row.into_summary_row())));
        }

        // Highest score first; recency then id break ties deterministically.
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.updated_at.cmp(&a.1.updated_at))
                .then_with(|| a.1.id.cmp(&b.1.id))
        });

        let total = scored.len() as i64;
        let recipes: Vec<RecipeSummary> = scored
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|(_, summary)| summary)
            .collect();

        return Ok(ListRecipesResponse {
            recipes,
            pagination: PaginationMetadata {
                total,
                limit,
                offset,
            },
        });
    }

    // Database text ordering is plain byte order, which isn't what users
    // expect and which the offline iOS cache would have to mirror. Apply the shared locale-independent
    // comparator in memory so cached and server-backed title sorts agree.
    if matches!(sort_by, SortBy::Title) {
        let mut title_rows: Vec<(Uuid, String)> = query
            .select((recipes::id, recipe_versions::title))
            .load(conn)
            .map_err(|error| {
                tracing::error!(?error, "Failed to fetch recipe titles for title sort");
                ApiError::internal("Failed to fetch recipes")
            })?;

        let descending = matches!(sort_dir, Direction::Desc);
        title_rows.sort_by(|lhs, rhs| {
            ramekin_core::recipe_title_sort::compare_recipe_titles(
                &lhs.1, &lhs.0 .0, &rhs.1, &rhs.0 .0, descending,
            )
        });

        let total = title_rows.len() as i64;
        let page_ids: Vec<Uuid> = title_rows
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|(id, _)| id)
            .collect();

        let rows: Vec<RecipeSummaryRow> = current_recipe_versions_for_user!(user_id)
            .filter(recipes::id.eq_any(&page_ids))
            .select(recipe_summary_select!())
            .load(conn)
            .map_err(|error| {
                tracing::error!(?error, "Failed to fetch title-sorted recipe page");
                ApiError::internal("Failed to fetch recipes")
            })?;
        let mut recipes: Vec<RecipeSummary> =
            rows.into_iter().map(RecipeSummary::from_row).collect();
        if recipes.len() != page_ids.len() {
            tracing::error!(
                expected = page_ids.len(),
                actual = recipes.len(),
                "Title-sorted recipe page changed while it was loading"
            );
            return Err(ApiError::internal("Failed to fetch recipes"));
        }
        let page_position_by_id: HashMap<Uuid, usize> = page_ids
            .into_iter()
            .enumerate()
            .map(|(position, id)| (id, position))
            .collect();
        recipes.sort_by_key(|recipe| page_position_by_id[&recipe.id]);

        return Ok(ListRecipesResponse {
            recipes,
            pagination: PaginationMetadata {
                total,
                limit,
                offset,
            },
        });
    }

    // Add ordering (with recipes::id tiebreaker for deterministic pagination)
    let query = match (sort_by, sort_dir) {
        (SortBy::Relevance, _) => unreachable!("relevance is handled above"),
        (SortBy::Title, _) => unreachable!("title is handled above"),
        (SortBy::Random, _) => query.order(raw_sql::random()),
        (SortBy::UpdatedAt, Direction::Desc) => {
            query.order((recipe_versions::created_at.desc(), recipes::id.asc()))
        }
        (SortBy::UpdatedAt, Direction::Asc) => {
            query.order((recipe_versions::created_at.asc(), recipes::id.asc()))
        }
        (SortBy::Rating, Direction::Desc) => query.order((
            recipe_versions::rating.is_null().asc(),
            recipe_versions::rating.desc(),
            recipes::id.asc(),
        )),
        (SortBy::Rating, Direction::Asc) => query.order((
            recipe_versions::rating.is_null().asc(),
            recipe_versions::rating.asc(),
            recipes::id.asc(),
        )),
        (SortBy::CreatedAt, Direction::Desc) => {
            query.order((recipes::created_at.desc(), recipes::id.asc()))
        }
        (SortBy::CreatedAt, Direction::Asc) => {
            query.order((recipes::created_at.asc(), recipes::id.asc()))
        }
        (SortBy::Calories, Direction::Desc) => query.order((
            recipe_versions::kcal_per_serving.is_null().asc(),
            recipe_versions::kcal_per_serving.desc(),
            recipes::id.asc(),
        )),
        (SortBy::Calories, Direction::Asc) => query.order((
            recipe_versions::kcal_per_serving.is_null().asc(),
            recipe_versions::kcal_per_serving.asc(),
            recipes::id.asc(),
        )),
    };

    // Select columns including COUNT(*) OVER() for total and tags via correlated subquery
    // All data fetched in a single query
    let results: Vec<CountedRecipeSummaryRow> = query
        .select(counted_recipe_summary_select!())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(|e| {
            tracing::error!("Failed to fetch recipes: {:?}", e);
            ApiError::internal("Failed to fetch recipes")
        })?;

    // The window count is unavailable when OFFSET leaves the page empty. Only
    // that case pays for a second query, using exactly the same filters.
    let total = if let Some(row) = results.first() {
        row.total
    } else {
        build_query().count().get_result(conn).map_err(|e| {
            tracing::error!("Failed to count recipes for empty page: {:?}", e);
            ApiError::internal("Failed to fetch recipes")
        })?
    };

    let recipes = results
        .into_iter()
        .map(|row| RecipeSummary::from_row(row.into_summary_row()))
        .collect();

    Ok(ListRecipesResponse {
        recipes,
        pagination: PaginationMetadata {
            total,
            limit,
            offset,
        },
    })
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
    fn test_parse_calorie_filters_match_clients() {
        #[derive(serde::Deserialize)]
        struct Vector {
            name: String,
            query: String,
            min: Option<f64>,
            max: Option<f64>,
        }
        let vectors: Vec<Vector> = serde_json::from_str(include_str!(
            "../../../../shared-test-vectors/calorie-filter.json"
        ))
        .unwrap();
        for vector in vectors {
            let parsed = parse_query(&vector.query);
            assert_eq!(
                (parsed.calories_min, parsed.calories_max),
                (vector.min, vector.max),
                "{}",
                vector.name
            );
            assert!(parsed.text.is_empty(), "{}", vector.name);
        }
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

    #[test]
    fn test_search_index_query_quotes_normalized_token() {
        assert_eq!(search_index_query("Crème").as_deref(), Some("\"creme\""));
        assert_eq!(
            search_index_query("say \"hi\" now").as_deref(),
            Some("\"say \"\"hi\"\" now\"")
        );
    }

    #[test]
    fn test_short_tokens_skip_the_search_index() {
        assert_eq!(search_index_query("ab"), None);
        assert_eq!(search_index_query("é"), None);
        // Normalization can lengthen a token past the trigram minimum.
        assert_eq!(search_index_query("Œu").as_deref(), Some("\"oeu\""));
    }
}
