//! SQL execution safety, lexical validation, and read-only mode enforcement.

use crate::db::error::{DbError, DbResult};

/// Validates SQL statements for safety and policy compliance.
pub struct QuerySafetyValidator;

impl QuerySafetyValidator {
    /// Strips leading/trailing whitespace and comments (-- line comments and /* block comments */).
    pub fn clean_sql(sql: &str) -> String {
        let mut result = String::with_capacity(sql.len());
        let mut chars = sql.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '-' && chars.peek() == Some(&'-') {
                // Line comment: skip until newline
                chars.next();
                for next_ch in chars.by_ref() {
                    if next_ch == '\n' {
                        result.push(' ');
                        break;
                    }
                }
            } else if ch == '/' && chars.peek() == Some(&'*') {
                // Block comment: skip until */
                chars.next();
                while let Some(b_ch) = chars.next() {
                    if b_ch == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        result.push(' ');
                        break;
                    }
                }
            } else {
                result.push(ch);
            }
        }

        result.trim().to_string()
    }

    /// Determines if a SQL query is a mutating statement (DML) or schema alteration (DDL).
    pub fn is_mutating_or_ddl(sql: &str) -> bool {
        let cleaned = Self::clean_sql(sql);
        if cleaned.is_empty() {
            return false;
        }

        let words: Vec<String> = cleaned
            .split(|c: char| c.is_whitespace() || c == ';' || c == '(' || c == ')')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_uppercase())
            .collect();

        if words.is_empty() {
            return false;
        }

        // Check prominent mutating or DDL keywords
        const MUTATING_KEYWORDS: &[&str] = &[
            "DROP", "TRUNCATE", "ALTER", "CREATE", "DELETE", "UPDATE", "INSERT",
            "REPLACE", "MERGE", "UPSERT", "GRANT", "REVOKE", "RENAME", "EXEC",
            "EXECUTE", "CALL", "VACUUM", "ATTACH", "DETACH",
        ];

        for word in &words {
            if MUTATING_KEYWORDS.contains(&word.as_str()) {
                return true;
            }
        }

        false
    }

    /// Check if statement is an unconstrained DELETE or UPDATE without WHERE clause.
    pub fn is_dangerous_unbounded_mutation(sql: &str) -> Option<&'static str> {
        let cleaned = Self::clean_sql(sql);
        let upper = cleaned.to_uppercase();

        let words: Vec<&str> = upper
            .split(|c: char| c.is_whitespace() || c == ';' || c == '(' || c == ')')
            .filter(|s| !s.is_empty())
            .collect();

        if words.is_empty() {
            return None;
        }

        let first = words[0];
        if (first == "DELETE" || first == "UPDATE") && !words.contains(&"WHERE") {
            if first == "DELETE" {
                return Some("Unbounded DELETE statement without a WHERE clause");
            } else {
                return Some("Unbounded UPDATE statement without a WHERE clause");
            }
        }

        None
    }

    /// Validate a SQL statement against connection safety policy.
    pub fn validate_query(sql: &str, is_read_only: bool) -> DbResult<()> {
        if is_read_only && Self::is_mutating_or_ddl(sql) {
            return Err(DbError::QueryExecution(
                "Execution blocked: Destructive or mutating operation rejected by Read-Only connection policy."
                    .to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_sql_comments() {
        let sql = "-- header comment\nSELECT * FROM users; /* inline comment */";
        let cleaned = QuerySafetyValidator::clean_sql(sql);
        assert_eq!(cleaned, "SELECT * FROM users;");
    }

    #[test]
    fn test_read_only_blocks_mutations() {
        assert!(QuerySafetyValidator::validate_query("SELECT * FROM users", true).is_ok());
        assert!(QuerySafetyValidator::validate_query("EXPLAIN SELECT 1", true).is_ok());
        assert!(QuerySafetyValidator::validate_query("SHOW TABLES", true).is_ok());

        assert!(QuerySafetyValidator::validate_query("DROP TABLE users", true).is_err());
        assert!(QuerySafetyValidator::validate_query("DELETE FROM users WHERE id = 1", true).is_err());
        assert!(QuerySafetyValidator::validate_query("UPDATE users SET name = 'Bob'", true).is_err());
        assert!(QuerySafetyValidator::validate_query("CREATE TABLE t(x int)", true).is_err());
        assert!(QuerySafetyValidator::validate_query("TRUNCATE TABLE logs", true).is_err());

        // When read_only is false, mutations are allowed
        assert!(QuerySafetyValidator::validate_query("DROP TABLE users", false).is_ok());
    }

    #[test]
    fn test_unbounded_mutation_detection() {
        assert_eq!(
            QuerySafetyValidator::is_dangerous_unbounded_mutation("DELETE FROM orders"),
            Some("Unbounded DELETE statement without a WHERE clause")
        );
        assert_eq!(
            QuerySafetyValidator::is_dangerous_unbounded_mutation("DELETE FROM orders WHERE id = 1"),
            None
        );
        assert_eq!(
            QuerySafetyValidator::is_dangerous_unbounded_mutation("UPDATE orders SET status = 1"),
            Some("Unbounded UPDATE statement without a WHERE clause")
        );
        assert_eq!(
            QuerySafetyValidator::is_dangerous_unbounded_mutation("UPDATE orders SET status = 1 WHERE id = 2"),
            None
        );
    }
}
