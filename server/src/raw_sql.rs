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
