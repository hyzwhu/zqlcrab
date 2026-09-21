//! Context-aware SQL autocompletion engine and LSP CompletionProvider.
//!
//! Provides intelligent completions for:
//! - Standard ANSI and dialect-specific SQL keywords (SQLite, PostgreSQL, MySQL)
//! - Built-in SQL functions (aggregates, strings, null handling, date/time)
//! - Live schema metadata: databases, schemas, tables, and views
//! - Table-scoped columns triggered via dot syntax (e.g. `users.id`) and global column matching
//! - Common query templates & snippets

use crate::db::types::{ColumnInfo, DatabaseFamily, TableInfo};
use gpui_kit::component::input::{CompletionProvider, Rope, RopeExt};
use gpui_kit::gpui::{App, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Position, Range as LspRange, TextEdit,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Thread-safe cached metadata used for autocompletion suggestions.
#[derive(Debug, Clone, Default)]
pub struct SqlMetadataCache {
    /// Active database family (SQLite, PostgreSQL, MySQL)
    pub family: Option<DatabaseFamily>,
    /// Available tables and views
    pub tables: Vec<TableInfo>,
    /// Column definitions mapped by lowercase table name
    pub columns_by_table: HashMap<String, Vec<ColumnInfo>>,
    /// Known schemas or databases
    pub schemas: Vec<String>,
}

impl SqlMetadataCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_family(&mut self, family: Option<DatabaseFamily>) {
        self.family = family;
    }

    pub fn set_tables(&mut self, tables: Vec<TableInfo>) {
        self.tables = tables;
    }

    pub fn set_databases(&mut self, databases: Vec<String>) {
        self.schemas = databases;
    }

    pub fn set_columns_for_table(&mut self, table_name: &str, columns: Vec<ColumnInfo>) {
        self.columns_by_table
            .insert(table_name.to_lowercase(), columns);
    }

    pub fn clear(&mut self) {
        self.family = None;
        self.tables.clear();
        self.columns_by_table.clear();
        self.schemas.clear();
    }
}

/// Autocomplete provider implementing `gpui_kit::component::input::CompletionProvider`.
pub struct SqlCompletionProvider {
    cache: Arc<RwLock<SqlMetadataCache>>,
}

impl SqlCompletionProvider {
    pub fn new(cache: Arc<RwLock<SqlMetadataCache>>) -> Self {
        Self { cache }
    }
}

/// Standard ANSI SQL keywords
const ANSI_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "INSERT INTO",
    "UPDATE",
    "DELETE FROM",
    "JOIN",
    "LEFT JOIN",
    "RIGHT JOIN",
    "INNER JOIN",
    "CROSS JOIN",
    "FULL OUTER JOIN",
    "ON",
    "GROUP BY",
    "ORDER BY",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "UNION",
    "UNION ALL",
    "INTERSECT",
    "EXCEPT",
    "DISTINCT",
    "AS",
    "SET",
    "VALUES",
    "AND",
    "OR",
    "NOT",
    "IN",
    "BETWEEN",
    "LIKE",
    "IS NULL",
    "IS NOT NULL",
    "EXISTS",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "ASC",
    "DESC",
    "CREATE TABLE",
    "DROP TABLE",
    "ALTER TABLE",
    "CREATE INDEX",
    "DROP INDEX",
    "PRIMARY KEY",
    "FOREIGN KEY",
    "REFERENCES",
    "DEFAULT",
    "NOT NULL",
    "UNIQUE",
    "CHECK",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
];

/// SQLite specific keywords
const SQLITE_KEYWORDS: &[&str] = &[
    "AUTOINCREMENT",
    "PRAGMA",
    "GLOB",
    "REGEXP",
    "VACUUM",
    "EXPLAIN QUERY PLAN",
    "ROWID",
    "WITHOUT ROWID",
];

/// PostgreSQL specific keywords
const POSTGRES_KEYWORDS: &[&str] = &[
    "ILIKE",
    "RETURNING",
    "ON CONFLICT",
    "DO NOTHING",
    "DO UPDATE",
    "SERIAL",
    "BIGSERIAL",
    "GENERATE_SERIES",
    "TRUNCATE",
    "CASCADE",
    "RESTRICT",
    "EXPLAIN ANALYZE",
];

/// MySQL specific keywords
const MYSQL_KEYWORDS: &[&str] = &[
    "AUTO_INCREMENT",
    "DESCRIBE",
    "EXPLAIN ANALYZE",
    "USE",
    "ON DUPLICATE KEY UPDATE",
    "IF NOT EXISTS",
    "SHOW TABLES",
    "SHOW DATABASES",
];

/// Built-in SQL functions and their details
const SQL_FUNCTIONS: &[(&str, &str, &str)] = &[
    ("COUNT", "COUNT(expression)", "Counts rows or non-null values"),
    ("SUM", "SUM(expression)", "Calculates the sum of values"),
    ("AVG", "AVG(expression)", "Calculates the average of values"),
    ("MIN", "MIN(expression)", "Returns the minimum value"),
    ("MAX", "MAX(expression)", "Returns the maximum value"),
    ("COALESCE", "COALESCE(val1, val2, ...)", "Returns the first non-null argument"),
    ("IFNULL", "IFNULL(expr1, expr2)", "Returns expr2 if expr1 is null"),
    ("NULLIF", "NULLIF(expr1, expr2)", "Returns null if expr1 equals expr2"),
    ("CONCAT", "CONCAT(s1, s2, ...)", "Concatenates string expressions"),
    ("SUBSTR", "SUBSTR(str, pos, len)", "Extracts a substring from text"),
    ("LENGTH", "LENGTH(str)", "Returns string character or byte length"),
    ("UPPER", "UPPER(str)", "Converts string to uppercase"),
    ("LOWER", "LOWER(str)", "Converts string to lowercase"),
    ("TRIM", "TRIM(str)", "Trims leading and trailing whitespace"),
    ("ROUND", "ROUND(num, decimals)", "Rounds a numeric value"),
    ("ABS", "ABS(num)", "Returns the absolute value"),
    ("NOW", "NOW()", "Returns the current date and time"),
    ("CURRENT_TIMESTAMP", "CURRENT_TIMESTAMP", "Current transaction timestamp"),
    ("DATE", "DATE(expression)", "Extracts or formats date"),
];

/// SQL Snippets
const SQL_SNIPPETS: &[(&str, &str, &str)] = &[
    ("sel", "SELECT * FROM ", "SELECT query snippet"),
    ("ins", "INSERT INTO ", "INSERT statement snippet"),
    ("upd", "UPDATE  SET ", "UPDATE statement snippet"),
    ("del", "DELETE FROM ", "DELETE statement snippet"),
    ("crt", "CREATE TABLE ", "CREATE TABLE snippet"),
];

/// Helper to analyze text at offset to find the trigger character and current word prefix.
#[derive(Debug, PartialEq, Eq)]
pub enum CompletionTriggerTarget {
    /// Dot completion on a specific table/alias (e.g. `users.`)
    Dot(String, String), // (table_name_or_alias, column_prefix)
    /// Normal word completion (e.g. `SEL`)
    Word(String),
}

/// Parse text preceding cursor offset into a CompletionTriggerTarget and start position.
pub fn parse_completion_prefix(rope: &Rope, offset: usize) -> (CompletionTriggerTarget, usize) {
    let mut left = 0;
    // Scan backward up to 64 chars to find word boundary or dot
    while left < 64 && offset > left {
        let idx = offset - left - 1;
        match rope.char_at(idx) {
            Some(c) if c.is_alphanumeric() || c == '_' || c == '.' || c == '`' || c == '"' => {
                left += 1;
            }
            _ => break,
        }
    }

    let start = offset - left;
    let token = rope.slice(start..offset).to_string();

    if let Some((table, col_prefix)) = token.rsplit_once('.') {
        let clean_table = table.trim_matches(|c| c == '`' || c == '"' || c == '[' || c == ']');
        (
            CompletionTriggerTarget::Dot(clean_table.to_string(), col_prefix.to_string()),
            start + table.len() + 1,
        )
    } else {
        let clean_token = token.trim_matches(|c| c == '`' || c == '"' || c == '[' || c == ']');
        (CompletionTriggerTarget::Word(clean_token.to_string()), start)
    }
}

/// Generate completion candidates based on parsed trigger and cache.
pub fn compute_completions(
    cache: &SqlMetadataCache,
    target: CompletionTriggerTarget,
    start_pos: Position,
    end_pos: Position,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    match target {
        CompletionTriggerTarget::Dot(table_ref, col_prefix) => {
            let col_prefix_lower = col_prefix.to_lowercase();
            let table_ref_lower = table_ref.to_lowercase();

            // Find matching table columns in cache
            if let Some(cols) = cache.columns_by_table.get(&table_ref_lower) {
                for col in cols {
                    if col.name.to_lowercase().starts_with(&col_prefix_lower) {
                        items.push(CompletionItem {
                            label: col.name.clone(),
                            filter_text: Some(col.name.clone()),
                            kind: Some(CompletionItemKind::FIELD),
                            detail: Some(format!("{} · {}", col.data_type, table_ref)),
                            documentation: col
                                .description
                                .as_ref()
                                .map(|c| lsp_types::Documentation::String(c.clone())),
                            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                range: LspRange {
                                    start: start_pos,
                                    end: end_pos,
                                },
                                new_text: col.name.clone(),
                            })),
                            ..Default::default()
                        });
                    }
                }
            }
        }
        CompletionTriggerTarget::Word(word_prefix) => {
            let prefix_lower = word_prefix.to_lowercase();
            if prefix_lower.is_empty() {
                return items;
            }

            // 1. Tables & Views
            for tbl in &cache.tables {
                let tbl_lower = tbl.name.to_lowercase();
                if tbl_lower.starts_with(&prefix_lower) {
                    let is_view = tbl.is_view();
                    let kind = if is_view {
                        CompletionItemKind::INTERFACE
                    } else {
                        CompletionItemKind::STRUCT
                    };
                    let detail = if let Some(ref schema) = tbl.schema {
                        format!("{} (schema: {})", tbl.table_type, schema)
                    } else {
                        tbl.table_type.clone()
                    };

                    items.push(CompletionItem {
                        label: tbl.name.clone(),
                        filter_text: Some(tbl.name.clone()),
                        kind: Some(kind),
                        detail: Some(detail),
                        documentation: tbl
                            .comment
                            .as_ref()
                            .map(|c| lsp_types::Documentation::String(c.clone())),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: LspRange {
                                start: start_pos,
                                end: end_pos,
                            },
                            new_text: tbl.name.clone(),
                        })),
                        ..Default::default()
                    });
                }
            }

            // 2. Global known columns across cached tables
            for (tbl_name, cols) in &cache.columns_by_table {
                for col in cols {
                    if col.name.to_lowercase().starts_with(&prefix_lower) {
                        items.push(CompletionItem {
                            label: col.name.clone(),
                            filter_text: Some(col.name.clone()),
                            kind: Some(CompletionItemKind::FIELD),
                            detail: Some(format!("Column in {} ({})", tbl_name, col.data_type)),
                            documentation: col
                                .description
                                .as_ref()
                                .map(|c| lsp_types::Documentation::String(c.clone())),
                            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                range: LspRange {
                                    start: start_pos,
                                    end: end_pos,
                                },
                                new_text: col.name.clone(),
                            })),
                            ..Default::default()
                        });
                    }
                }
            }

            // 3. SQL Keywords (ANSI + Dialect)
            let mut keywords: Vec<&str> = ANSI_KEYWORDS.to_vec();
            match cache.family {
                Some(DatabaseFamily::Sqlite) => keywords.extend_from_slice(SQLITE_KEYWORDS),
                Some(DatabaseFamily::Postgres) => keywords.extend_from_slice(POSTGRES_KEYWORDS),
                Some(DatabaseFamily::MySql) => keywords.extend_from_slice(MYSQL_KEYWORDS),
                None => {}
            }

            for kw in keywords {
                if kw.to_lowercase().starts_with(&prefix_lower) {
                    items.push(CompletionItem {
                        label: kw.to_string(),
                        filter_text: Some(kw.to_lowercase()),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some("SQL Keyword".to_string()),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: LspRange {
                                start: start_pos,
                                end: end_pos,
                            },
                            new_text: kw.to_string(),
                        })),
                        ..Default::default()
                    });
                }
            }

            // 4. Built-in SQL Functions
            for (fn_name, signature, doc) in SQL_FUNCTIONS {
                if fn_name.to_lowercase().starts_with(&prefix_lower) {
                    let insert_text = if signature.ends_with("()") {
                        format!("{}()", fn_name)
                    } else {
                        format!("{}(", fn_name)
                    };
                    items.push(CompletionItem {
                        label: fn_name.to_string(),
                        filter_text: Some(fn_name.to_lowercase()),
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some(signature.to_string()),
                        documentation: Some(lsp_types::Documentation::String(doc.to_string())),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: LspRange {
                                start: start_pos,
                                end: end_pos,
                            },
                            new_text: insert_text,
                        })),
                        ..Default::default()
                    });
                }
            }

            // 5. Snippets
            for (prefix, snippet, desc) in SQL_SNIPPETS {
                if prefix.starts_with(&prefix_lower) {
                    items.push(CompletionItem {
                        label: prefix.to_string(),
                        filter_text: Some(prefix.to_string()),
                        kind: Some(CompletionItemKind::SNIPPET),
                        detail: Some(desc.to_string()),
                        documentation: Some(lsp_types::Documentation::String(snippet.to_string())),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: LspRange {
                                start: start_pos,
                                end: end_pos,
                            },
                            new_text: snippet.to_string(),
                        })),
                        ..Default::default()
                    });
                }
            }
        }
    }

    // Deduplicate items by label and kind
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| {
        let key = (item.label.clone(), format!("{:?}", item.kind));
        seen.insert(key)
    });

    items
}

impl CompletionProvider for SqlCompletionProvider {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        _cx: &mut App,
    ) -> Task<gpui_kit::gpui::Result<CompletionResponse>> {
        let (target, replace_start) = parse_completion_prefix(text, offset);

        if match &target {
            CompletionTriggerTarget::Word(w) => w.is_empty(),
            // When user types `ecrm_yb.` with empty col prefix, allow suggestions so all columns popup!
            CompletionTriggerTarget::Dot(tbl, _) => tbl.is_empty(),
        } {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        }

        let start_pos = text.offset_to_position(replace_start);
        let end_pos = text.offset_to_position(offset);

        let cache = if let Ok(guard) = self.cache.read() {
            guard.clone()
        } else {
            SqlMetadataCache::default()
        };

        let items = compute_completions(&cache, target, start_pos, end_pos);
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        // Trigger completion on alphanumeric character, underscore, or dot
        new_text.chars().any(|c| c.is_alphanumeric() || c == '_' || c == '.')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::ColumnInfo;

    #[test]
    fn test_parse_completion_prefix_word() {
        let rope = Rope::from("SELECT u FROM");
        let (target, start) = parse_completion_prefix(&rope, 8);
        assert_eq!(target, CompletionTriggerTarget::Word("u".to_string()));
        assert_eq!(start, 7);

        let rope = Rope::from("SELECT users FROM");
        let (target, start) = parse_completion_prefix(&rope, 12);
        assert_eq!(target, CompletionTriggerTarget::Word("users".to_string()));
        assert_eq!(start, 7);
    }

    #[test]
    fn test_parse_completion_prefix_dot() {
        let rope = Rope::from("SELECT users.id FROM");
        // offset 15 is immediately after "SELECT users.id"
        let (target, start) = parse_completion_prefix(&rope, 15);
        assert_eq!(
            target,
            CompletionTriggerTarget::Dot("users".to_string(), "id".to_string())
        );
        assert_eq!(start, 13);
    }

    #[test]
    fn test_compute_completions_keywords() {
        let cache = SqlMetadataCache::default();
        let target = CompletionTriggerTarget::Word("sel".to_string());
        let pos = Position::new(0, 0);
        let completions = compute_completions(&cache, target, pos, pos);

        assert!(
            completions
                .iter()
                .any(|c| c.label == "SELECT" && c.kind == Some(CompletionItemKind::KEYWORD))
        );
    }

    #[test]
    fn test_compute_completions_tables_and_columns() {
        let mut cache = SqlMetadataCache::default();
        cache.set_tables(vec![TableInfo {
            name: "customers".to_string(),
            schema: None,
            table_type: "BASE TABLE".to_string(),
            comment: Some("Customer records".to_string()),
            row_count_estimate: None,
        }]);
        cache.set_columns_for_table(
            "customers",
            vec![
                ColumnInfo {
                    name: "id".to_string(),
                    data_type: "INTEGER".to_string(),
                    is_primary_key: true,
                    is_nullable: false,
                    is_auto_increment: true,
                    default_value: None,
                    description: None,
                },
                ColumnInfo {
                    name: "email".to_string(),
                    data_type: "TEXT".to_string(),
                    is_primary_key: false,
                    is_nullable: false,
                    is_auto_increment: false,
                    default_value: None,
                    description: None,
                },
            ],
        );

        // Word match on table
        let target_tbl = CompletionTriggerTarget::Word("cust".to_string());
        let pos = Position::new(0, 0);
        let res_tbl = compute_completions(&cache, target_tbl, pos, pos);
        assert!(
            res_tbl
                .iter()
                .any(|c| c.label == "customers" && c.kind == Some(CompletionItemKind::STRUCT))
        );

        // Dot match on table column with specific prefix
        let target_col =
            CompletionTriggerTarget::Dot("customers".to_string(), "em".to_string());
        let res_col = compute_completions(&cache, target_col, pos, pos);
        assert_eq!(res_col.len(), 1);
        assert_eq!(res_col[0].label, "email");
        assert_eq!(res_col[0].kind, Some(CompletionItemKind::FIELD));

        // Dot match on table with EMPTY column prefix (typing `table.` immediately)
        let target_empty_col =
            CompletionTriggerTarget::Dot("customers".to_string(), "".to_string());
        let res_empty_col = compute_completions(&cache, target_empty_col, pos, pos);
        assert_eq!(res_empty_col.len(), 2);
        assert!(res_empty_col.iter().any(|c| c.label == "id"));
        assert!(res_empty_col.iter().any(|c| c.label == "email"));

        // Quoted table reference `customers`.
        let (parsed_target, _) = parse_completion_prefix(&Rope::from("SELECT `customers`."), 19);
        assert_eq!(
            parsed_target,
            CompletionTriggerTarget::Dot("customers".to_string(), "".to_string())
        );
    }

    #[test]
    fn test_dialect_specific_keywords() {
        let mut cache_sqlite = SqlMetadataCache::default();
        cache_sqlite.set_family(Some(DatabaseFamily::Sqlite));
        let res_sqlite = compute_completions(
            &cache_sqlite,
            CompletionTriggerTarget::Word("prag".to_string()),
            Position::new(0, 0),
            Position::new(0, 0),
        );
        assert!(res_sqlite.iter().any(|c| c.label == "PRAGMA"));

        let mut cache_pg = SqlMetadataCache::default();
        cache_pg.set_family(Some(DatabaseFamily::Postgres));
        let res_pg = compute_completions(
            &cache_pg,
            CompletionTriggerTarget::Word("ili".to_string()),
            Position::new(0, 0),
            Position::new(0, 0),
        );
        assert!(res_pg.iter().any(|c| c.label == "ILIKE"));
    }
}
