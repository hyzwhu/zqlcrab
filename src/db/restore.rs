//! SQL Script Restore and Batch Execution Engine.
//! Provides robust SQL statement parsing, classification, error policies, and real-time execution metrics.

use serde::{Deserialize, Serialize};

/// Error handling policy when executing multi-statement SQL batch scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RestoreErrorPolicy {
    #[default]
    StopOnError,     // Immediately abort on first failure
    ContinueAndLog,  // Continue executing remaining statements and log failures
}

impl RestoreErrorPolicy {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::StopOnError => "Stop on First Error",
            Self::ContinueAndLog => "Continue & Log Errors",
        }
    }
}

/// Execution options for script restoration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreOptions {
    pub error_policy: RestoreErrorPolicy,
    pub wrap_in_transaction: bool,
    pub disable_foreign_keys: bool,
}

impl Default for RestoreOptions {
    fn default() -> Self {
        Self {
            error_policy: RestoreErrorPolicy::StopOnError,
            wrap_in_transaction: true,
            disable_foreign_keys: true,
        }
    }
}

/// Analysis metadata of a target SQL script file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SqlScriptAnalysis {
    pub file_path: Option<String>,
    pub file_size_bytes: usize,
    pub total_statements: usize,
    pub ddl_count: usize,
    pub dml_count: usize,
    pub other_count: usize,
    pub preview_statements: Vec<String>,
}

/// Real-time statement execution log record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementExecutionLog {
    pub index: usize,
    pub sql_preview: String,
    pub is_success: bool,
    pub error_message: Option<String>,
    pub duration_ms: u64,
}

/// Live progress notification emitted during script restore execution.
#[derive(Debug, Clone)]
pub struct RestoreProgress {
    pub current_statement: usize,
    pub total_statements: usize,
    pub percent: f32,
    pub succeeded_count: usize,
    pub failed_count: usize,
    pub current_sql_preview: String,
    pub is_finished: bool,
}

/// Aggregated summary upon batch restore execution completion.
#[derive(Debug, Clone, Default)]
pub struct RestoreSummary {
    pub total_statements: usize,
    pub succeeded_count: usize,
    pub failed_count: usize,
    pub duration_ms: u128,
    pub aborted_early: bool,
    pub logs: Vec<StatementExecutionLog>,
}

/// Splits a raw SQL script into individual executable statements.
/// Accurately ignores semicolons inside single quotes, double quotes, block comments, and line comments.
pub fn split_sql_script(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::with_capacity(256);
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;

    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while i < len {
        let ch = chars[i];
        let next_ch = if i + 1 < len { Some(chars[i + 1]) } else { None };

        // 1. Line comment handling (-- or #)
        if in_line_comment {
            current.push(ch);
            if ch == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        // 2. Block comment handling (/* ... */)
        if in_block_comment {
            current.push(ch);
            if ch == '*' && next_ch == Some('/') {
                current.push('/');
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        // 3. String literals (single quotes '...')
        if in_single_quote {
            current.push(ch);
            if ch == '\'' {
                // Check for escaped quote ''
                if next_ch == Some('\'') {
                    current.push('\'');
                    i += 2;
                    continue;
                }
                in_single_quote = false;
            } else if ch == '\\' && next_ch.is_some() {
                // Escaped character like \'
                if let Some(nc) = next_ch {
                    current.push(nc);
                    i += 2;
                    continue;
                }
            }
            i += 1;
            continue;
        }

        // 4. Double quote identifiers ("...")
        if in_double_quote {
            current.push(ch);
            if ch == '"' {
                if next_ch == Some('"') {
                    current.push('"');
                    i += 2;
                    continue;
                }
                in_double_quote = false;
            }
            i += 1;
            continue;
        }

        // 5. Backtick identifiers (`...`)
        if in_backtick {
            current.push(ch);
            if ch == '`' {
                in_backtick = false;
            }
            i += 1;
            continue;
        }

        // 6. Check transitions into comments / quotes
        if ch == '-' && next_ch == Some('-') {
            in_line_comment = true;
            current.push('-');
            current.push('-');
            i += 2;
            continue;
        } else if ch == '#' {
            in_line_comment = true;
            current.push('#');
            i += 1;
            continue;
        } else if ch == '/' && next_ch == Some('*') {
            in_block_comment = true;
            current.push('/');
            current.push('*');
            i += 2;
            continue;
        } else if ch == '\'' {
            in_single_quote = true;
            current.push('\'');
            i += 1;
            continue;
        } else if ch == '"' {
            in_double_quote = true;
            current.push('"');
            i += 1;
            continue;
        } else if ch == '`' {
            in_backtick = true;
            current.push('`');
            i += 1;
            continue;
        }

        // 7. Statement boundary (semicolon)
        if ch == ';' {
            let stmt = current.trim();
            if !stmt.is_empty() {
                statements.push(stmt.to_string());
            }
            current.clear();
            i += 1;
            continue;
        }

        current.push(ch);
        i += 1;
    }

    // Trailing statement without trailing semicolon
    let trailing = current.trim();
    if !trailing.is_empty() {
        statements.push(trailing.to_string());
    }

    // Filter out statements that are pure comments with no executable SQL
    statements
        .into_iter()
        .filter(|s| {
            let cleaned = strip_comments(s);
            !cleaned.trim().is_empty()
        })
        .collect()
}

/// Strips comments from statement string for empty-check and classification.
fn strip_comments(stmt: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in stmt.lines() {
        let trimmed = line.trim();
        if in_block {
            if let Some(idx) = trimmed.find("*/") {
                out.push_str(&trimmed[idx + 2..]);
                in_block = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            if let Some(idx) = trimmed.find("*/") {
                out.push_str(&trimmed[idx + 2..]);
            } else {
                in_block = true;
            }
            continue;
        }
        if trimmed.starts_with("--") || trimmed.starts_with('#') {
            continue;
        }
        out.push_str(line);
        out.push(' ');
    }
    out
}

/// Classifies and analyzes a SQL script for UI inspection before execution.
pub fn analyze_sql_script(sql: &str, file_path: Option<String>) -> SqlScriptAnalysis {
    let statements = split_sql_script(sql);
    let mut ddl_count = 0;
    let mut dml_count = 0;
    let mut other_count = 0;
    let mut preview_statements = Vec::new();

    for (idx, stmt) in statements.iter().enumerate() {
        let clean = strip_comments(stmt).trim().to_uppercase();
        if clean.starts_with("CREATE")
            || clean.starts_with("ALTER")
            || clean.starts_with("DROP")
            || clean.starts_with("TRUNCATE")
            || clean.starts_with("RENAME")
        {
            ddl_count += 1;
        } else if clean.starts_with("INSERT")
            || clean.starts_with("UPDATE")
            || clean.starts_with("DELETE")
            || clean.starts_with("REPLACE")
            || clean.starts_with("MERGE")
        {
            dml_count += 1;
        } else {
            other_count += 1;
        }

        if idx < 5 {
            // Keep first 5 statements for preview
            let preview = if stmt.len() > 160 {
                format!("{}...", &stmt[..160])
            } else {
                stmt.clone()
            };
            preview_statements.push(preview);
        }
    }

    SqlScriptAnalysis {
        file_path,
        file_size_bytes: sql.len(),
        total_statements: statements.len(),
        ddl_count,
        dml_count,
        other_count,
        preview_statements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_sql_script_with_quotes_and_comments() {
        let sql = r#"
            -- First table
            CREATE TABLE test (
                id INT PRIMARY KEY,
                description TEXT DEFAULT 'Hello; World' -- semicolon in quote!
            );

            /* Block comment
               with semicolon ; inside */
            INSERT INTO test (id, description) VALUES (1, 'Value with ; semicolon and ''escaped'' quote');

            UPDATE test SET description = "double;quoted" WHERE id = 1;
        "#;

        let stmts = split_sql_script(sql);
        assert_eq!(stmts.len(), 3);
        assert!(stmts[0].contains("CREATE TABLE test"));
        assert!(stmts[1].contains("INSERT INTO test"));
        assert!(stmts[1].contains("Value with ; semicolon"));
        assert!(stmts[2].contains("UPDATE test"));
    }

    #[test]
    fn test_analyze_sql_script_classification() {
        let sql = r#"
            CREATE TABLE a (id INT);
            CREATE INDEX idx_a ON a(id);
            INSERT INTO a VALUES (1);
            UPDATE a SET id = 2 WHERE id = 1;
            DELETE FROM a WHERE id = 2;
            PRAGMA foreign_keys = OFF;
            DROP TABLE a;
        "#;

        let analysis = analyze_sql_script(sql, Some("backup.sql".to_string()));
        assert_eq!(analysis.total_statements, 7);
        assert_eq!(analysis.ddl_count, 3); // CREATE TABLE, CREATE INDEX, DROP TABLE
        assert_eq!(analysis.dml_count, 3); // INSERT, UPDATE, DELETE
        assert_eq!(analysis.other_count, 1); // PRAGMA
        assert_eq!(analysis.preview_statements.len(), 5);
    }

    #[test]
    fn test_split_sql_script_ignores_pure_comments() {
        let sql = r#"
            -- Just a comment
            -- Another comment
            /* multi line
               comment */
            SELECT 1;
            -- trailing comment
        "#;

        let stmts = split_sql_script(sql);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("SELECT 1"));
    }
}
