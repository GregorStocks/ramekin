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
use diesel::sql_types::{Array, Bool, Text};

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

/// ILIKE filter on the ingredients JSONB field cast to text.
///
/// Diesel has no native support for casting JSONB to text for ILIKE.
///
/// # Safety
/// The pattern is embedded in the SQL string with single-quote escaping
/// (`'` → `''`). Callers must pass a pattern already processed by
/// `escape_like_pattern` (which handles `\`, `%`, `_`).
pub fn ingredients_ilike(pattern: &str) -> SqlLiteral<Bool> {
    let sql_escaped = pattern.replace('\'', "''");
    sql::<Bool>(&format!(
        "recipe_versions.ingredients::text ILIKE '{}'",
        sql_escaped
    ))
}
