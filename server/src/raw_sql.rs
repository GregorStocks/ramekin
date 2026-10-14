//! Raw SQL fragments that can't be expressed in Diesel's type-safe DSL.
//!
//! # Safety
//!
//! All SQL in this module has been reviewed for SQL injection safety:
//! - User input is passed via `.bind()` parameters or with proper escaping
//! - See per-function safety docs for details
//!
//! When adding new SQL here:
//! 1. Document why Diesel DSL can't be used
//! 2. Ensure all user input uses `.bind()`
//! 3. Update scripts/sql_allowlist.txt with the new pattern

use crate::sql_types::UuidList;
use diesel::deserialize::{self, FromSql};
use diesel::dsl::sql;
use diesel::expression::SqlLiteral;
use diesel::sql_types::{Bool, Text};
use diesel::sqlite::{Sqlite, SqliteValue};

diesel::define_sql_function! {
    /// SQLite json_array_length(): the number of photos in `photo_ids`.
    fn json_array_length(array: UuidList) -> diesel::sql_types::Integer;
}

diesel::define_sql_function! {
    /// SQLite random(): a random 64-bit integer, a valid sort key.
    fn random() -> diesel::sql_types::BigInt;
}

/// A list of strings returned as a JSON array (`json_group_array`).
#[derive(diesel::sql_types::SqlType, diesel::query_builder::QueryId, Clone, Copy, Debug)]
#[diesel(sqlite_type(name = "Text"))]
pub struct TextList;

impl FromSql<TextList, Sqlite> for Vec<String> {
    fn from_sql(value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
        Ok(serde_json::from_str(&text)?)
    }
}

/// Correlated subquery to fetch tags for the current recipe_versions row.
///
/// Returns the tag names from user_tags via the junction table, ordered
/// case-insensitively. Diesel doesn't support correlated subqueries with
/// aggregates natively.
///
/// # Safety
/// Static SQL string with no user input. References recipe_versions.id
/// from the outer query context.
pub fn tags_subquery() -> SqlLiteral<TextList> {
    sql::<TextList>(
        "(SELECT json_group_array(ut.name ORDER BY ut.name_key, ut.name) \
         FROM recipe_version_tags rvt \
         JOIN user_tags ut ON ut.id = rvt.tag_id \
         WHERE rvt.recipe_version_id = recipe_versions.id \
         AND ut.deleted_at IS NULL)",
    )
}

/// EXISTS filter: at least one photo on the current recipe version matches a
/// file-size threshold (bytes).
///
/// # Safety
/// The threshold is an `i32` and is formatted directly into the SQL string.
/// Integers cannot cause SQL injection.
pub fn photo_file_size_filter(op: &'static str, bytes: i32) -> SqlLiteral<Bool> {
    debug_assert!(op == "<" || op == ">");
    sql::<Bool>(&format!(
        "EXISTS (SELECT 1 FROM photos p \
         WHERE p.id IN (SELECT value FROM json_each(recipe_versions.photo_ids)) \
         AND p.deleted_at IS NULL \
         AND p.file_size IS NOT NULL \
         AND p.file_size {} {})",
        op, bytes
    ))
}

/// EXISTS filter: at least one photo on the current recipe version has a
/// minimum dimension (smaller of width/height) matching a threshold.
///
/// # Safety
/// The threshold is an `i32` and is formatted directly into the SQL string.
/// Integers cannot cause SQL injection.
pub fn photo_min_dim_filter(op: &'static str, pixels: i32) -> SqlLiteral<Bool> {
    debug_assert!(op == "<" || op == ">");
    sql::<Bool>(&format!(
        "EXISTS (SELECT 1 FROM photos p \
         WHERE p.id IN (SELECT value FROM json_each(recipe_versions.photo_ids)) \
         AND p.deleted_at IS NULL \
         AND p.width IS NOT NULL AND p.height IS NOT NULL \
         AND min(p.width, p.height) {} {})",
        op, pixels
    ))
}

/// Filter: the recipe's search index row (its current version's normalized
/// text, see the `recipe_search` table) matches an FTS5 query.
///
/// The unary `+` keeps SQLite from probing the version index once per
/// (recipe, match) pair instead of reading the match set once.
///
/// # Safety
/// The query is bound as a parameter.
pub fn recipe_search_match(
    fts_query: String,
) -> SqlLiteral<
    Bool,
    diesel::expression::UncheckedBind<
        SqlLiteral<Bool>,
        diesel::internal::derives::as_expression::Bound<Text, String>,
    >,
> {
    sql::<Bool>("+recipes.seq IN (SELECT rowid FROM recipe_search WHERE recipe_search MATCH ")
        .bind::<Text, _>(fts_query)
        .sql(")")
}

/// The ingredient haystack bare-text search matches against: the stored
/// ingredients JSON text, both indexed for the search filter and served as
/// `ingredient_match_text` through recipe sync, so iOS local search matches
/// the same string. Selected as raw text so the JSON isn't reserialized.
///
/// # Safety
/// Static SQL string with no user input.
pub fn ingredients_text() -> SqlLiteral<Text> {
    sql::<Text>("recipe_versions.ingredients")
}
