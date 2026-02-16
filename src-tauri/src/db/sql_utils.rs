/// Build a comma-separated string of `?` placeholders for use in SQL `IN` clauses.
///
/// # Example
/// ```ignore
/// let placeholders = build_in_clause_placeholders(3);
/// assert_eq!(placeholders, "?,?,?");
/// ```
pub fn build_in_clause_placeholders(count: usize) -> String {
    let mut s = String::with_capacity(count * 2);
    for i in 0..count {
        if i > 0 {
            s.push(',');
        }
        s.push('?');
    }
    s
}
