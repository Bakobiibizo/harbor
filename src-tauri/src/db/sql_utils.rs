//! Shared SQL utility functions for safe dynamic query construction.
//!
//! These helpers exist to avoid using `format!()` with user-supplied data in SQL
//! queries.  Every function in this module returns **only** static SQL fragments
//! (e.g. `"?,?,?"`) that are safe to interpolate into a query string.  All
//! actual values must still be bound via parameterized placeholders.

/// Build a comma-separated string of `?` placeholders for use in SQL `IN (...)` clauses.
///
/// # Safety (SQL injection)
///
/// The returned string contains **only** literal `?` characters and commas -- no
/// user-supplied data is ever included.  The actual values corresponding to these
/// placeholders must be bound separately via rusqlite's parameter-binding API
/// (e.g. `params![]`, `params_from_iter`, etc.).
///
/// # Panics
///
/// Panics if `count` is zero, since `IN ()` is invalid SQL.
///
/// # Example
///
/// ```ignore
/// let placeholders = build_in_clause_placeholders(3);
/// assert_eq!(placeholders, "?,?,?");
/// let sql = format!("SELECT * FROM t WHERE id IN ({})", placeholders);
/// // bind actual values via params_from_iter
/// ```
pub fn build_in_clause_placeholders(count: usize) -> String {
    assert!(count > 0, "Cannot build IN clause with zero placeholders");
    let placeholders: Vec<&str> = (0..count).map(|_| "?").collect();
    placeholders.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_placeholder() {
        assert_eq!(build_in_clause_placeholders(1), "?");
    }

    #[test]
    fn test_multiple_placeholders() {
        assert_eq!(build_in_clause_placeholders(3), "?,?,?");
    }

    #[test]
    fn test_large_count() {
        let result = build_in_clause_placeholders(5);
        assert_eq!(result, "?,?,?,?,?");
    }

    #[test]
    #[should_panic(expected = "Cannot build IN clause with zero placeholders")]
    fn test_zero_panics() {
        build_in_clause_placeholders(0);
    }
}
