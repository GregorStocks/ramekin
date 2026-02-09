//! Raw SQL fragments that can't be expressed in Diesel's type-safe DSL.
//!
//! # Safety
//!
//! All SQL in this module has been reviewed for SQL injection safety:
//! - User input is ALWAYS passed via `.bind()` parameters
//! - No string concatenation or interpolation with user data
//!
//! When adding new SQL here:
//! 1. Document why Diesel DSL can't be used
//! 2. Ensure all user input uses `.bind()`
//! 3. Update scripts/sql_allowlist.txt with the new pattern

use diesel::dsl::sql;
use diesel::expression::SqlLiteral;
use diesel::sql_types::{Array, Text};

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
