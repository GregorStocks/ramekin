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
use diesel::sql_types::{Array, BigInt, Text};

/// Window function for counting total rows across the full result set.
///
/// Returns `COUNT(*) OVER()` which gives the total count before LIMIT/OFFSET.
/// Diesel doesn't support window functions natively.
///
/// # Safety
/// Static SQL string with no user input.
pub fn count_over() -> SqlLiteral<BigInt> {
    sql::<BigInt>("COUNT(*) OVER()")
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
         WHERE rvt.recipe_version_id = recipe_versions.id)",
    )
}

/// Correlated subquery to count recipes using a tag.
///
/// Counts distinct recipes where the tag is on the current_version and the
/// recipe is not deleted. Used by the tag list endpoint.
/// Diesel doesn't easily support chained LEFT JOINs with GROUP BY for counts.
///
/// # Safety
/// Static SQL string with no user input. References user_tags.id
/// from the outer query context.
pub fn tag_recipe_count() -> SqlLiteral<BigInt> {
    sql::<BigInt>(
        "(SELECT COUNT(DISTINCT r.id) \
         FROM recipe_version_tags rvt \
         JOIN recipe_versions rv ON rv.id = rvt.recipe_version_id \
         JOIN recipes r ON r.current_version_id = rv.id AND r.deleted_at IS NULL \
         WHERE rvt.tag_id = user_tags.id)",
    )
}
