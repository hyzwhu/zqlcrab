//! SQL statement generator for atomic tabular updates and deletions across database dialects.

use crate::db::changeset::GridChangeset;
use crate::db::types::{quote_ident, ColumnInfo, DatabaseFamily, QueryValue};
use std::collections::BTreeMap;

/// Detailed review plan generated from pending changeset modifications.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlReviewPlan {
    pub table_name: String,
    pub schema_name: Option<String>,
    pub database_family: DatabaseFamily,
    pub inserts_count: usize,
    pub updates_count: usize,
    pub deletes_count: usize,
    pub has_primary_key: bool,
    pub primary_keys: Vec<String>,
    pub warnings: Vec<String>,
    pub statements: Vec<String>,
    pub full_script: String,
}

/// Generates a structured SQL review plan and transaction script for pending changeset edits.
pub fn generate_review_plan(
    table_name: &str,
    schema_name: Option<&str>,
    family: DatabaseFamily,
    columns: &[ColumnInfo],
    grid_columns: &[String],
    original_rows: &[Vec<QueryValue>],
    changeset: &GridChangeset,
) -> SqlReviewPlan {
    let mut warnings = Vec::new();
    let mut statements = Vec::new();

    // 1. Identify primary keys from schema metadata
    let pk_names: Vec<String> = columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| c.name.clone())
        .collect();

    let has_primary_key = !pk_names.is_empty();

    if !has_primary_key && changeset.is_dirty() {
        warnings.push(format!(
            "Table '{table_name}' has no primary key. WHERE conditions will match all original column values, which may affect duplicate rows if any exist."
        ));
    }

    // Qualified table identifier (e.g. "public"."users" or `mydb`.`users`)
    let qualified_table = match schema_name {
        Some(s) if !s.trim().is_empty() => {
            format!("{}.{}", quote_ident(s, family), quote_ident(table_name, family))
        }
        _ => quote_ident(table_name, family),
    };

    // 2. Generate INSERT statements for uncommitted new rows
    let mut inserts_count = 0;
    for insertion in &changeset.inserted_rows {
        let mut insert_cols = Vec::new();
        let mut insert_vals = Vec::new();

        for (col_idx, col_name) in grid_columns.iter().enumerate() {
            let val = insertion.values.get(col_idx).unwrap_or(&QueryValue::Null);
            let col_meta = columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name));

            // Determine whether to omit an auto-increment or serial column when value is Null or <auto>
            let is_auto = col_meta.map(|c| {
                c.is_auto_increment
                    || c.data_type.to_lowercase().contains("serial")
                    || (c.is_primary_key && (c.data_type.to_lowercase().contains("int") || family == DatabaseFamily::Sqlite))
            }).unwrap_or(false);

            let is_auto_placeholder = match val {
                QueryValue::Null => is_auto,
                QueryValue::String(s) if s.trim().eq_ignore_ascii_case("<auto>") => true,
                _ => false,
            };

            if is_auto_placeholder {
                continue;
            }

            insert_cols.push(quote_ident(col_name, family));
            insert_vals.push(format_query_value(val, family));
        }

        if insert_cols.is_empty() {
            match family {
                DatabaseFamily::MySql => {
                    statements.push(format!("INSERT INTO {qualified_table} () VALUES ();"));
                }
                _ => {
                    statements.push(format!("INSERT INTO {qualified_table} DEFAULT VALUES;"));
                }
            }
        } else {
            let cols_str = insert_cols.join(", ");
            let vals_str = insert_vals.join(", ");
            // Multi-line formatting if wide or many columns to avoid modal viewport horizontal sprawl
            if cols_str.len() + vals_str.len() > 80 || insert_cols.len() > 5 {
                let indented_cols = insert_cols
                    .iter()
                    .map(|c| format!("    {c}"))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let indented_vals = insert_vals
                    .iter()
                    .map(|v| format!("    {v}"))
                    .collect::<Vec<_>>()
                    .join(",\n");
                statements.push(format!(
                    "INSERT INTO {qualified_table} (\n{indented_cols}\n) VALUES (\n{indented_vals}\n);"
                ));
            } else {
                statements.push(format!(
                    "INSERT INTO {qualified_table} ({cols_str}) VALUES ({vals_str});"
                ));
            }
        }
        inserts_count += 1;
    }

    // 3. Group cell updates by row index (skip rows that are also marked deleted)
    let mut row_updates: BTreeMap<usize, Vec<(usize, String, QueryValue)>> = BTreeMap::new();
    for (&(r_idx, c_idx), edit) in &changeset.cell_updates {
        if changeset.is_row_deleted(r_idx) {
            continue; // Deletion takes precedence
        }
        row_updates
            .entry(r_idx)
            .or_default()
            .push((c_idx, edit.column_name.clone(), edit.new_value.clone()));
    }

    let mut updates_count = 0;

    // 3. Generate UPDATE statements
    for (r_idx, edits) in row_updates {
        let Some(orig_row) = original_rows.get(r_idx) else {
            continue;
        };

        // SET clause
        let mut set_clauses = Vec::new();
        for (_, col_name, new_val) in edits {
            let col_quoted = quote_ident(&col_name, family);
            let val_str = format_query_value(&new_val, family);
            set_clauses.push(format!("{col_quoted} = {val_str}"));
        }

        // WHERE clause
        let where_clause = build_row_where_clause(
            family,
            columns,
            grid_columns,
            orig_row,
            &pk_names,
        );

        let set_str = set_clauses.join(", ");
        statements.push(format!(
            "UPDATE {qualified_table}\nSET {set_str}\nWHERE {where_clause};"
        ));
        updates_count += 1;
    }

    // 4. Generate DELETE statements
    let mut deletes_count = 0;
    // Sort deletion keys for deterministic output
    let mut del_indices: Vec<usize> = changeset.deleted_rows.keys().copied().collect();
    del_indices.sort_unstable();

    for r_idx in del_indices {
        let del_info = &changeset.deleted_rows[&r_idx];
        let where_clause = build_row_where_clause(
            family,
            columns,
            grid_columns,
            &del_info.original_row,
            &pk_names,
        );

        statements.push(format!("DELETE FROM {qualified_table}\nWHERE {where_clause};"));
        deletes_count += 1;
    }

    // 6. Wrap inside an atomic transaction script
    let full_script = build_transaction_script(family, &statements);

    SqlReviewPlan {
        table_name: table_name.to_string(),
        schema_name: schema_name.map(|s| s.to_string()),
        database_family: family,
        inserts_count,
        updates_count,
        deletes_count,
        has_primary_key,
        primary_keys: pk_names,
        warnings,
        statements,
        full_script,
    }
}

/// Constructs the WHERE clause identifying a specific row based on Primary Key or full row fallback.
fn build_row_where_clause(
    family: DatabaseFamily,
    columns: &[ColumnInfo],
    grid_columns: &[String],
    row_values: &[QueryValue],
    pk_names: &[String],
) -> String {
    let mut clauses = Vec::new();

    if !pk_names.is_empty() {
        // Match only primary key columns
        for pk in pk_names {
            // Find column index
            let col_idx = grid_columns.iter().position(|c| c.eq_ignore_ascii_case(pk))
                .or_else(|| columns.iter().position(|c| c.name.eq_ignore_ascii_case(pk)));

            if let Some(idx) = col_idx {
                if let Some(val) = row_values.get(idx) {
                    let col_quoted = quote_ident(pk, family);
                    if val.is_null() {
                        clauses.push(format!("{col_quoted} IS NULL"));
                    } else {
                        clauses.push(format!("{col_quoted} = {}", format_query_value(val, family)));
                    }
                }
            }
        }
    }

    // If no PK was found or couldn't match PK values, match all non-blob original column values
    if clauses.is_empty() {
        for (idx, col_name) in grid_columns.iter().enumerate() {
            if let Some(val) = row_values.get(idx) {
                // Skip blobs in WHERE matching fallback
                if matches!(val, QueryValue::Bytes(_)) {
                    continue;
                }
                let col_quoted = quote_ident(col_name, family);
                if val.is_null() {
                    clauses.push(format!("{col_quoted} IS NULL"));
                } else {
                    clauses.push(format!("{col_quoted} = {}", format_query_value(val, family)));
                }
            }
        }
    }

    if clauses.is_empty() {
        "1 = 1".to_string()
    } else {
        clauses.join(" AND ")
    }
}

/// Formats a QueryValue as a dialect-safe SQL literal.
pub fn format_query_value(val: &QueryValue, family: DatabaseFamily) -> String {
    match val {
        QueryValue::Null => "NULL".to_string(),
        QueryValue::Bool(b) => match family {
            DatabaseFamily::Postgres => if *b { "TRUE" } else { "FALSE" }.to_string(),
            DatabaseFamily::MySql | DatabaseFamily::Sqlite => if *b { "1" } else { "0" }.to_string(),
        },
        QueryValue::Int(i) => i.to_string(),
        QueryValue::Float(f) => {
            let s = format!("{f}");
            if s.contains('.') {
                s
            } else {
                format!("{f}.0")
            }
        }
        QueryValue::String(s) => match family {
            DatabaseFamily::MySql => {
                let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
                format!("'{escaped}'")
            }
            DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
                let escaped = s.replace('\'', "''");
                format!("'{escaped}'")
            }
        },
        QueryValue::DateTime(dt) => {
            let escaped = dt.replace('\'', "''");
            format!("'{escaped}'")
        }
        QueryValue::Bytes(b) => match family {
            DatabaseFamily::Postgres => format!("'\\x{}'::bytea", hex_encode(b)),
            DatabaseFamily::MySql | DatabaseFamily::Sqlite => format!("X'{}'", hex_encode(b)),
        },
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02X}", b);
    }
    s
}

/// Encloses statements within an atomic transaction.
fn build_transaction_script(family: DatabaseFamily, statements: &[String]) -> String {
    if statements.is_empty() {
        return "-- No pending changes to execute".to_string();
    }

    let joined_stmts = statements.join("\n\n");

    match family {
        DatabaseFamily::MySql => {
            format!(
                "START TRANSACTION;\n\n{joined_stmts}\n\nCOMMIT;"
            )
        }
        DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
            format!(
                "BEGIN;\n\n{joined_stmts}\n\nCOMMIT;"
            )
        }
    }
}

/// Extracts the target table name (and optional schema name) from a SQL query string.
/// Supports SELECT ... FROM, INSERT INTO, UPDATE, and DELETE FROM queries across dialects.
pub fn extract_table_from_sql(sql: &str) -> Option<(Option<String>, String)> {
    let cleaned = crate::db::safety::QuerySafetyValidator::clean_sql(sql);
    if cleaned.is_empty() {
        return None;
    }

    let words: Vec<&str> = cleaned.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    let mut target_token: Option<&str> = None;

    for (i, &word) in words.iter().enumerate() {
        let upper = word.to_ascii_uppercase();
        let stripped_upper = upper.trim_matches(|c: char| !c.is_alphanumeric());

        if stripped_upper == "FROM" || stripped_upper == "INTO" || stripped_upper == "UPDATE" {
            let mut next_idx = i + 1;
            // Skip optional keywords like ONLY (Postgres: SELECT * FROM ONLY users)
            if next_idx < words.len() && words[next_idx].eq_ignore_ascii_case("ONLY") {
                next_idx += 1;
            }
            if next_idx < words.len() {
                target_token = Some(words[next_idx]);
                break;
            }
        }
    }

    let raw_target = target_token?;
    let trimmed = raw_target.trim_matches(|c| c == ';' || c == ',' || c == ')' || c == '(').trim();
    if trimmed.is_empty() || trimmed.starts_with('(') {
        return None;
    }

    parse_qualified_identifier(trimmed)
}

fn parse_qualified_identifier(ident: &str) -> Option<(Option<String>, String)> {
    let clean_ident = ident.trim();
    if clean_ident.is_empty() {
        return None;
    }

    // Split on '.' while handling quotes around parts
    let parts: Vec<&str> = clean_ident.split('.').collect();
    if parts.len() == 1 {
        let tbl = strip_identifier_quotes(parts[0]);
        if tbl.is_empty() {
            None
        } else {
            Some((None, tbl))
        }
    } else if parts.len() == 2 {
        let schema = strip_identifier_quotes(parts[0]);
        let tbl = strip_identifier_quotes(parts[1]);
        if tbl.is_empty() {
            None
        } else {
            Some((if schema.is_empty() { None } else { Some(schema) }, tbl))
        }
    } else {
        let schema = strip_identifier_quotes(parts[parts.len() - 2]);
        let tbl = strip_identifier_quotes(parts[parts.len() - 1]);
        if tbl.is_empty() {
            None
        } else {
            Some((if schema.is_empty() { None } else { Some(schema) }, tbl))
        }
    }
}

fn strip_identifier_quotes(s: &str) -> String {
    s.trim()
        .trim_matches(|c| c == '`' || c == '"' || c == '\'' || c == '[' || c == ']')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::ColumnInfo;

    fn sample_columns() -> Vec<ColumnInfo> {
        vec![
            ColumnInfo {
                name: "id".into(),
                data_type: "INTEGER".into(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: true,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "name".into(),
                data_type: "TEXT".into(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "note".into(),
                data_type: "TEXT".into(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ]
    }

    #[test]
    fn test_generate_update_and_delete_with_primary_key() {
        let cols = sample_columns();
        let grid_cols = vec!["id".to_string(), "name".to_string(), "note".to_string()];
        let orig_rows = vec![
            vec![QueryValue::Int(1), QueryValue::String("Alice".into()), QueryValue::Null],
            vec![QueryValue::Int(2), QueryValue::String("Bob".into()), QueryValue::String("Hello".into())],
        ];

        let mut cs = GridChangeset::new();
        // Update row 0's name
        cs.set_cell_value(
            0,
            1,
            "name".into(),
            QueryValue::String("Alice".into()),
            QueryValue::String("Alice Smith".into()),
        );
        // Delete row 1
        cs.mark_row_deleted(1, &orig_rows[1]);

        let plan = generate_review_plan(
            "users",
            Some("public"),
            DatabaseFamily::Postgres,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        assert!(plan.has_primary_key);
        assert_eq!(plan.primary_keys, vec!["id"]);
        assert_eq!(plan.updates_count, 1);
        assert_eq!(plan.deletes_count, 1);
        assert!(plan.warnings.is_empty());

        let script = plan.full_script;
        assert!(script.starts_with("BEGIN;"));
        assert!(script.ends_with("COMMIT;"));
        assert!(script.contains("UPDATE \"public\".\"users\""));
        assert!(script.contains("SET \"name\" = 'Alice Smith'"));
        assert!(script.contains("WHERE \"id\" = 1;"));
        assert!(script.contains("DELETE FROM \"public\".\"users\""));
        assert!(script.contains("WHERE \"id\" = 2;"));
    }

    #[test]
    fn test_generate_review_plan_without_primary_key_fallback() {
        let cols = vec![
            ColumnInfo {
                name: "tag".into(),
                data_type: "TEXT".into(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "active".into(),
                data_type: "INTEGER".into(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];
        let grid_cols = vec!["tag".to_string(), "active".to_string()];
        let orig_rows = vec![
            vec![QueryValue::Null, QueryValue::Int(1)],
        ];

        let mut cs = GridChangeset::new();
        cs.set_cell_value(
            0,
            0,
            "tag".into(),
            QueryValue::Null,
            QueryValue::String("dev's test".into()),
        );

        let plan = generate_review_plan(
            "logs",
            None,
            DatabaseFamily::Sqlite,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        assert!(!plan.has_primary_key);
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("no primary key"));

        let script = plan.full_script;
        assert!(script.contains("UPDATE \"logs\""));
        assert!(script.contains("SET \"tag\" = 'dev''s test'"));
        // WHERE clause matches all columns including NULL handling
        assert!(script.contains("WHERE \"tag\" IS NULL AND \"active\" = 1;"));
    }

    #[test]
    fn test_mysql_transaction_and_escaping() {
        let cols = sample_columns();
        let grid_cols = vec!["id".to_string(), "name".to_string(), "note".to_string()];
        let orig_rows = vec![
            vec![QueryValue::Int(10), QueryValue::String("O'Reilly".into()), QueryValue::Null],
        ];

        let mut cs = GridChangeset::new();
        cs.set_cell_value(
            0,
            1,
            "name".into(),
            QueryValue::String("O'Reilly".into()),
            QueryValue::String("O'Reilly\\New".into()),
        );

        let plan = generate_review_plan(
            "authors",
            None,
            DatabaseFamily::MySql,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        let script = plan.full_script;
        assert!(script.starts_with("START TRANSACTION;"));
        assert!(script.contains("UPDATE `authors`"));
        assert!(script.contains("SET `name` = 'O\\'Reilly\\\\New'"));
        assert!(script.contains("WHERE `id` = 10;"));
    }

    #[test]
    fn test_insert_into_generation_sqlite_and_auto_increment() {
        let cols = sample_columns();
        let grid_cols = vec!["id".to_string(), "name".to_string(), "email".to_string(), "age".to_string(), "active".to_string()];
        let orig_rows = vec![];

        let mut cs = GridChangeset::new();
        // Row 1: id is Null (auto increment) -> should omit id
        cs.add_inserted_row(vec![
            QueryValue::Null,
            QueryValue::String("David".into()),
            QueryValue::String("david@example.com".into()),
            QueryValue::Int(32),
            QueryValue::Int(1),
        ]);
        // Row 2: id is explicitly specified -> should include id
        cs.add_inserted_row(vec![
            QueryValue::Int(99),
            QueryValue::String("Eve".into()),
            QueryValue::Null,
            QueryValue::Null,
            QueryValue::Int(0),
        ]);

        let plan = generate_review_plan(
            "users",
            None,
            DatabaseFamily::Sqlite,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        assert_eq!(plan.inserts_count, 2);
        assert_eq!(plan.updates_count, 0);
        assert_eq!(plan.deletes_count, 0);

        let script = plan.full_script;
        assert!(script.starts_with("BEGIN;"));
        assert!(script.contains("INSERT INTO \"users\" (\"name\", \"email\", \"age\", \"active\") VALUES ('David', 'david@example.com', 32, 1);"));
        assert!(script.contains("INSERT INTO \"users\" (\"id\", \"name\", \"email\", \"age\", \"active\") VALUES (99, 'Eve', NULL, NULL, 0);"));
        assert!(script.ends_with("COMMIT;"));
    }

    #[test]
    fn test_insert_into_generation_mysql_dialect() {
        let cols = sample_columns();
        let grid_cols = vec!["id".to_string(), "name".to_string()];
        let orig_rows = vec![];

        let mut cs = GridChangeset::new();
        cs.add_inserted_row(vec![
            QueryValue::String("<auto>".into()),
            QueryValue::String("Frank's \"Gadgets\"".into()),
        ]);

        let plan = generate_review_plan(
            "products",
            Some("shop_db"),
            DatabaseFamily::MySql,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        assert_eq!(plan.inserts_count, 1);
        let script = plan.full_script;
        assert!(script.starts_with("START TRANSACTION;"));
        assert!(script.contains("INSERT INTO `shop_db`.`products` (`name`) VALUES ('Frank\\'s \"Gadgets\"');"));
    }

    #[test]
    fn test_multiline_insert_formatting_for_wide_tables() {
        let cols: Vec<ColumnInfo> = (1..=10)
            .map(|i| ColumnInfo {
                name: format!("col_{i}"),
                data_type: "VARCHAR(255)".into(),
                is_nullable: true,
                is_primary_key: i == 1,
                is_auto_increment: false,
                default_value: None,
                description: None,
            })
            .collect();
        let grid_cols: Vec<String> = (1..=10).map(|i| format!("col_{i}")).collect();
        let mut cs = GridChangeset::new();
        cs.add_inserted_row(vec![
            QueryValue::Int(1),
            QueryValue::String("very long descriptive text title".into()),
            QueryValue::String("detailed information content goes here".into()),
            QueryValue::String("value_4".into()),
            QueryValue::String("value_5".into()),
            QueryValue::String("value_6".into()),
            QueryValue::String("value_7".into()),
            QueryValue::String("value_8".into()),
            QueryValue::String("value_9".into()),
            QueryValue::String("value_10".into()),
        ]);

        let plan = generate_review_plan(
            "test_case",
            Some("skill_up_web"),
            DatabaseFamily::MySql,
            &cols,
            &grid_cols,
            &[],
            &cs,
        );

        let script = plan.full_script;
        assert!(script.contains("INSERT INTO `skill_up_web`.`test_case` ("));
        // Check that columns and values are indented across multiple lines
        assert!(script.contains("    `col_1`,\n    `col_2`,"));
        assert!(script.contains("    1,\n    'very long descriptive text title',"));
    }

    #[test]
    fn test_extract_table_from_sql_queries() {
        // MySQL with schema
        let res = extract_table_from_sql("SELECT * FROM `skill_up_web`.`test_case` LIMIT 100;");
        assert_eq!(res, Some((Some("skill_up_web".into()), "test_case".into())));

        // Postgres with double quotes
        let res = extract_table_from_sql("SELECT id, name FROM \"public\".\"users\" WHERE active = true;");
        assert_eq!(res, Some((Some("public".into()), "users".into())));

        // SQLite simple table
        let res = extract_table_from_sql("SELECT * FROM test_case;");
        assert_eq!(res, Some((None, "test_case".into())));

        // MS SQL brackets
        let res = extract_table_from_sql("SELECT * FROM [dbo].[Customers]");
        assert_eq!(res, Some((Some("dbo".into()), "Customers".into())));

        // Lowercase and whitespace
        let res = extract_table_from_sql("   select  id  from   accounts   where id = 10 ");
        assert_eq!(res, Some((None, "accounts".into())));

        // INSERT statement
        let res = extract_table_from_sql("INSERT INTO `orders` (id, amount) VALUES (1, 99.9);");
        assert_eq!(res, Some((None, "orders".into())));

        // UPDATE statement
        let res = extract_table_from_sql("UPDATE `mydb`.`items` SET status = 'done';");
        assert_eq!(res, Some((Some("mydb".into()), "items".into())));

        // Non-table queries
        assert_eq!(extract_table_from_sql("SELECT 1 + 1;"), None);
        assert_eq!(extract_table_from_sql("SHOW DATABASES;"), None);
    }
}
