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

use diesel::dsl::sql;
use diesel::expression::SqlLiteral;
use diesel::sql_types::{Array, Bool, Nullable, Text, Uuid as SqlUuid};

diesel::define_sql_function! {
    /// PostgreSQL cardinality() function for array length.
    fn cardinality(array: Array<Nullable<SqlUuid>>) -> diesel::sql_types::Integer;
}

diesel::define_sql_function! {
    /// PostgreSQL lower() function.
    fn lower(x: diesel::sql_types::Text) -> diesel::sql_types::Text;
}

diesel::define_sql_function! {
    /// PostgreSQL random() function.
    fn random() -> diesel::sql_types::Double;
}

diesel::define_sql_function! {
    /// The writing transaction's own 64-bit id, used to stamp sync-visible
    /// changes. Defined in the `sync_change_xid` migration.
    fn current_change_xid() -> diesel::sql_types::BigInt;
}

diesel::define_sql_function! {
    /// The lowest transaction id still in flight for the current snapshot.
    /// Every change stamped below it is settled, so it is a race-safe sync
    /// cursor. Defined in the `sync_change_xid` migration.
    fn change_xid_watermark() -> diesel::sql_types::BigInt;
}

/// Correlated subquery to fetch tags for the current recipe_versions row.
///
/// Returns an array of tag names from user_tags via the junction table.
/// Diesel doesn't support correlated subqueries with array_agg natively.
///
/// # Safety
/// Static SQL string with no user input. References recipe_versions.id
/// from the outer query context.
pub fn tags_subquery() -> SqlLiteral<Array<Text>> {
    sql::<Array<Text>>(
        "(SELECT COALESCE(array_agg(ut.name ORDER BY ut.name), ARRAY[]::text[]) \
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
         WHERE p.id = ANY(recipe_versions.photo_ids) \
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
         WHERE p.id = ANY(recipe_versions.photo_ids) \
         AND p.deleted_at IS NULL \
         AND p.width IS NOT NULL AND p.height IS NOT NULL \
         AND LEAST(p.width, p.height) {} {})",
        op, pixels
    ))
}

/// Accent- and case-insensitive ILIKE filter on a text column.
///
/// Emits `f_unaccent(<column>) ILIKE f_unaccent('<pattern>')` so that queries
/// for "creme brulee" match rows containing "crème brûlée". Matches the
/// expression trigram indexes defined on `f_unaccent(col)`.
///
/// # Safety
/// `column` is a compile-time `&'static str` that must be a literal column
/// reference (e.g. `recipe_versions.title`) — never user input. The pattern
/// is embedded with single-quote escaping (`'` → `''`) and callers must have
/// already run it through `escape_like_pattern` (which handles `\`, `%`, `_`).
pub fn unaccent_ilike(column: &'static str, pattern: &str) -> SqlLiteral<Bool> {
    let sql_escaped = pattern.replace('\'', "''");
    sql::<Bool>(&format!(
        "f_unaccent({}) ILIKE f_unaccent('{}')",
        column, sql_escaped
    ))
}

/// The ingredients JSONB cast to text: the haystack bare-text search matches
/// against, both as the filter below and as the `ingredient_match_text`
/// served through recipe sync. One expression so they can never diverge.
const INGREDIENTS_TEXT_SQL: &str = "recipe_versions.ingredients::text";

/// Accent- and case-insensitive ILIKE filter on the ingredients JSONB field
/// cast to text. Diesel has no native support for casting JSONB to text for
/// ILIKE.
///
/// # Safety
/// See `unaccent_ilike` — same pattern escaping rules apply.
pub fn ingredients_unaccent_ilike(pattern: &str) -> SqlLiteral<Bool> {
    unaccent_ilike(INGREDIENTS_TEXT_SQL, pattern)
}

/// The exact haystack `ingredients_unaccent_ilike` matches against. Served
/// through recipe sync so the iOS local search matches the same string
/// instead of trying to re-create PostgreSQL's JSONB serialization. Diesel
/// has no native support for casting JSONB to text.
///
/// # Safety
/// Static SQL string with no user input.
pub fn ingredients_text() -> SqlLiteral<Text> {
    sql::<Text>(INGREDIENTS_TEXT_SQL)
}
