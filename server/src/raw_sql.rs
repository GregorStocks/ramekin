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
use diesel::sql_types::BigInt;

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

/// SQL fragment for case-insensitive tag containment check.
///
/// # Usage
/// ```ignore
/// sql::<Bool>(raw_sql::TAG_ARRAY_PREFIX)
///     .bind::<Text, _>(tag)
///     .sql(raw_sql::TAG_ARRAY_SUFFIX)
/// ```
///
/// # Safety
/// The tag value MUST be passed via `.bind()`, not interpolated.
///
/// # Why raw SQL?
/// Diesel doesn't have native support for citext array containment.
pub const TAG_ARRAY_PREFIX: &str = "(";
pub const TAG_ARRAY_SUFFIX: &str = "::citext = ANY(recipe_versions.tags))";

/// Query to get distinct tags for a user's recipes.
///
/// Uses `unnest()` to expand the tags array, which isn't in Diesel's DSL.
///
/// # Safety
/// The user_id MUST be passed via `.bind()`, not interpolated.
pub const DISTINCT_TAGS_QUERY: &str = "SELECT DISTINCT unnest(rv.tags)::text AS tag \
    FROM recipes r \
    JOIN recipe_versions rv ON rv.id = r.current_version_id \
    WHERE r.user_id = $1 AND r.deleted_at IS NULL \
    ORDER BY tag";
