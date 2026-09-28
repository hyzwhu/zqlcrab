//! Table data discrepancy detection and multi-dialect DML synchronization script generator.
//!
//! Compares rows between two tables (Source and Target) based on primary key or composite unique keys,
//! detecting Insertions, Deletions, Modifications, and Identical records, and producing dialect-accurate
//! DML synchronization scripts for PostgreSQL, MySQL, and SQLite.

use crate::db::schema_diff::MigrationDirection;
use crate::db::types::{DatabaseFamily, QueryValue, quote_ident};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Comparison status for an individual row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataDiffStatus {
    /// Row exists in Source table but is missing in Target table (+ INSERT).
    Added,
    /// Row exists in Target table but is missing in Source table (- DELETE).
    Deleted,
    /// Row exists in both tables with matching key, but one or more column values differ (~ UPDATE).
    Modified,
    /// Row exists in both tables and all column values are identical (= IDENTICAL).
    Identical,
}

impl DataDiffStatus {
    pub fn is_different(&self) -> bool {
        !matches!(self, Self::Identical)
    }

    pub fn badge_label(&self) -> &'static str {
        match self {
            Self::Added => "+ ADD",
            Self::Deleted => "- DROP",
            Self::Modified => "~ MODIFY",
            Self::Identical => "= IDENTICAL",
        }
    }
}

/// Column-level value discrepancy for a modified row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellDiff {
    pub column: String,
    pub source_value: QueryValue,
    pub target_value: QueryValue,
    pub source_display: String,
    pub target_display: String,
}

/// Row discrepancy record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowDiff {
    /// Printable summary of the row's primary key (e.g. `id = 101` or `pk1 = 1, pk2 = 'A'`).
    pub key_summary: String,
    pub status: DataDiffStatus,
    pub source_row: Option<Vec<QueryValue>>,
    pub target_row: Option<Vec<QueryValue>>,
    pub cell_diffs: Vec<CellDiff>,
    pub key_values: Vec<(String, QueryValue)>,
}

/// Aggregated metrics of data discrepancies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataDiffSummary {
    pub total_source_rows: usize,
    pub total_target_rows: usize,
    pub added_count: usize,
    pub deleted_count: usize,
    pub modified_count: usize,
    pub identical_count: usize,
}

impl DataDiffSummary {
    pub fn total_differences(&self) -> usize {
        self.added_count + self.deleted_count + self.modified_count
    }

    pub fn has_differences(&self) -> bool {
        self.total_differences() > 0
    }
}

/// Configuration options for data comparison and sync script generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDiffOptions {
    pub direction: MigrationDirection,
    /// Whether to generate destructive DELETE statements. If true, DELETE statements are emitted.
    /// If false (Safe Mode), DELETE statements are commented out with `-- [SAFE MODE SKIPPED]`.
    pub safe_mode: bool,
    /// Whether to wrap the generated statements in a transaction (BEGIN / COMMIT).
    pub wrap_transaction: bool,
    /// Maximum rows to compare.
    pub max_rows: usize,
}

impl Default for DataDiffOptions {
    fn default() -> Self {
        Self {
            direction: MigrationDirection::SourceToTarget,
            safe_mode: true,
            wrap_transaction: true,
            max_rows: 1000,
        }
    }
}

/// Comprehensive comparison report between two tables' data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataDiffReport {
    pub source_table: String,
    pub target_table: String,
    pub columns: Vec<String>,
    pub key_columns: Vec<String>,
    pub rows: Vec<RowDiff>,
    pub summary: DataDiffSummary,
    pub dialect: DatabaseFamily,
}

impl DataDiffReport {
    pub fn empty(source_table: String, target_table: String, dialect: DatabaseFamily) -> Self {
        Self {
            source_table,
            target_table,
            columns: Vec::new(),
            key_columns: Vec::new(),
            rows: Vec::new(),
            summary: DataDiffSummary::default(),
            dialect,
        }
    }
}

/// Generated DML synchronization script with metrics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSyncScript {
    pub sql: String,
    pub insert_count: usize,
    pub update_count: usize,
    pub delete_count: usize,
    pub total_statements: usize,
    pub warnings: Vec<String>,
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Formats a `QueryValue` as a dialect-compatible SQL literal.
pub fn format_query_value_literal(val: &QueryValue, dialect: DatabaseFamily) -> String {
    match val {
        QueryValue::Null => "NULL".to_string(),
        QueryValue::Bool(b) => match dialect {
            DatabaseFamily::Postgres => {
                if *b {
                    "TRUE".to_string()
                } else {
                    "FALSE".to_string()
                }
            }
            DatabaseFamily::MySql | DatabaseFamily::Sqlite => {
                if *b {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
        },
        QueryValue::Int(i) => i.to_string(),
        QueryValue::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                "NULL".to_string()
            } else {
                format!("{}", f)
            }
        }
        QueryValue::String(s) => {
            let escaped = s.replace('\'', "''");
            format!("'{}'", escaped)
        }
        QueryValue::DateTime(d) => {
            let escaped = d.replace('\'', "''");
            format!("'{}'", escaped)
        }
        QueryValue::Bytes(b) => match dialect {
            DatabaseFamily::Postgres => format!("'\\x{}'", hex_encode(b)),
            DatabaseFamily::MySql | DatabaseFamily::Sqlite => format!("X'{}'", hex_encode(b)),
        },
    }
}

/// Compares row records between Source and Target tables.
pub fn compare_table_data(
    source_table: &str,
    target_table: &str,
    columns: &[String],
    key_columns: &[String],
    source_rows: &[Vec<QueryValue>],
    target_rows: &[Vec<QueryValue>],
    dialect: DatabaseFamily,
) -> DataDiffReport {
    let mut effective_keys: Vec<String> = key_columns
        .iter()
        .filter(|k| columns.iter().any(|c| c == *k))
        .cloned()
        .collect();

    // Fallback: if no valid key column is supplied, use the first column if available
    if effective_keys.is_empty() && !columns.is_empty() {
        effective_keys.push(columns[0].clone());
    }

    let key_indices: Vec<usize> = effective_keys
        .iter()
        .filter_map(|k| columns.iter().position(|c| c == k))
        .collect();

    let make_canonical_key = |row: &[QueryValue]| -> String {
        key_indices
            .iter()
            .map(|&idx| {
                row.get(idx)
                    .map(|v| v.to_display_string())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("\x1F")
    };

    let make_key_summary = |row: &[QueryValue]| -> (String, Vec<(String, QueryValue)>) {
        let mut pairs = Vec::with_capacity(key_indices.len());
        let mut summary_parts = Vec::with_capacity(key_indices.len());
        for &idx in &key_indices {
            let col_name = columns.get(idx).cloned().unwrap_or_default();
            let val = row.get(idx).cloned().unwrap_or(QueryValue::Null);
            summary_parts.push(format!("{} = {}", col_name, val.to_display_string()));
            pairs.push((col_name, val));
        }
        (summary_parts.join(", "), pairs)
    };

    // Index target rows by canonical key
    let mut target_map: HashMap<String, (usize, &Vec<QueryValue>)> =
        HashMap::with_capacity(target_rows.len());
    for (idx, row) in target_rows.iter().enumerate() {
        let k = make_canonical_key(row);
        target_map.insert(k, (idx, row));
    }

    let mut matched_target_indices: HashSet<usize> = HashSet::with_capacity(target_rows.len());
    let mut diff_rows = Vec::new();
    let mut summary = DataDiffSummary {
        total_source_rows: source_rows.len(),
        total_target_rows: target_rows.len(),
        ..Default::default()
    };

    // Process source rows
    for s_row in source_rows {
        let k = make_canonical_key(s_row);
        let (key_summary, key_values) = make_key_summary(s_row);

        if let Some(&(t_idx, t_row)) = target_map.get(&k) {
            matched_target_indices.insert(t_idx);

            // Compare column-by-column
            let mut cell_diffs = Vec::new();
            for (col_idx, col_name) in columns.iter().enumerate() {
                let s_val = s_row.get(col_idx).cloned().unwrap_or(QueryValue::Null);
                let t_val = t_row.get(col_idx).cloned().unwrap_or(QueryValue::Null);
                if s_val != t_val {
                    cell_diffs.push(CellDiff {
                        column: col_name.clone(),
                        source_display: s_val.to_display_string(),
                        target_display: t_val.to_display_string(),
                        source_value: s_val,
                        target_value: t_val,
                    });
                }
            }

            if cell_diffs.is_empty() {
                summary.identical_count += 1;
                diff_rows.push(RowDiff {
                    key_summary,
                    status: DataDiffStatus::Identical,
                    source_row: Some(s_row.clone()),
                    target_row: Some(t_row.clone()),
                    cell_diffs: Vec::new(),
                    key_values,
                });
            } else {
                summary.modified_count += 1;
                diff_rows.push(RowDiff {
                    key_summary,
                    status: DataDiffStatus::Modified,
                    source_row: Some(s_row.clone()),
                    target_row: Some(t_row.clone()),
                    cell_diffs,
                    key_values,
                });
            }
        } else {
            // Source row does not exist in target
            summary.added_count += 1;
            diff_rows.push(RowDiff {
                key_summary,
                status: DataDiffStatus::Added,
                source_row: Some(s_row.clone()),
                target_row: None,
                cell_diffs: Vec::new(),
                key_values,
            });
        }
    }

    // Process remaining target rows (Deleted in source)
    for (t_idx, t_row) in target_rows.iter().enumerate() {
        if !matched_target_indices.contains(&t_idx) {
            let (key_summary, key_values) = make_key_summary(t_row);
            summary.deleted_count += 1;
            diff_rows.push(RowDiff {
                key_summary,
                status: DataDiffStatus::Deleted,
                source_row: None,
                target_row: Some(t_row.clone()),
                cell_diffs: Vec::new(),
                key_values,
            });
        }
    }

    DataDiffReport {
        source_table: source_table.to_string(),
        target_table: target_table.to_string(),
        columns: columns.to_vec(),
        key_columns: effective_keys,
        rows: diff_rows,
        summary,
        dialect,
    }
}

/// Generates a complete DML synchronization SQL script for the report and options.
pub fn generate_data_sync_sql(
    report: &DataDiffReport,
    options: &DataDiffOptions,
) -> DataSyncScript {
    let mut script = String::new();
    let mut insert_count = 0;
    let mut update_count = 0;
    let mut delete_count = 0;
    let mut warnings = Vec::new();

    let (src_tbl, dest_tbl) = match options.direction {
        MigrationDirection::SourceToTarget => (&report.source_table, &report.target_table),
        MigrationDirection::TargetToSource => (&report.target_table, &report.source_table),
    };

    if report.key_columns.is_empty() {
        warnings.push(
            "No primary key or unique identity specified; update/delete clauses may affect ambiguous rows."
                .to_string(),
        );
    }

    let dialect_label = match report.dialect {
        DatabaseFamily::Postgres => "PostgreSQL",
        DatabaseFamily::MySql => "MySQL",
        DatabaseFamily::Sqlite => "SQLite",
    };

    // Header banner
    script.push_str(&format!(
        "-- ==========================================================================\n\
         -- zqlcrab Data Synchronization Script\n\
         -- Target Dialect: {}\n\
         -- Sync Direction: {} ➔ {}\n\
         -- Generated: {}\n\
         -- Differences: +{} Inserts, ~{} Updates, -{} Deletes\n\
         -- ==========================================================================\n\n",
        dialect_label,
        src_tbl,
        dest_tbl,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        report.summary.added_count,
        report.summary.modified_count,
        report.summary.deleted_count
    ));

    if options.wrap_transaction {
        match report.dialect {
            DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
                script.push_str("BEGIN;\n\n");
            }
            DatabaseFamily::MySql => {
                script.push_str("START TRANSACTION;\n\n");
            }
        }
    }

    let quoted_dest_table = quote_ident(dest_tbl, report.dialect);

    let quoted_columns: Vec<String> = report
        .columns
        .iter()
        .map(|c| quote_ident(c, report.dialect))
        .collect();
    let col_list = quoted_columns.join(", ");

    let build_where_clause = |key_values: &[(String, QueryValue)]| -> String {
        if key_values.is_empty() {
            "1 = 1".to_string()
        } else {
            key_values
                .iter()
                .map(|(col, val)| {
                    let quoted_col = quote_ident(col, report.dialect);
                    let val_literal = format_query_value_literal(val, report.dialect);
                    if val.is_null() {
                        format!("{} IS NULL", quoted_col)
                    } else {
                        format!("{} = {}", quoted_col, val_literal)
                    }
                })
                .collect::<Vec<_>>()
                .join(" AND ")
        }
    };

    match options.direction {
        MigrationDirection::SourceToTarget => {
            // SourceToTarget:
            // Added (in source, missing in target) -> INSERT into target
            // Modified -> UPDATE target with source values
            // Deleted (missing in source, exists in target) -> DELETE from target
            for row in &report.rows {
                match row.status {
                    DataDiffStatus::Added => {
                        if let Some(ref s_row) = row.source_row {
                            let values_str = s_row
                                .iter()
                                .map(|v| format_query_value_literal(v, report.dialect))
                                .collect::<Vec<_>>()
                                .join(", ");
                            script.push_str(&format!(
                                "INSERT INTO {} ({}) VALUES ({});\n",
                                quoted_dest_table, col_list, values_str
                            ));
                            insert_count += 1;
                        }
                    }
                    DataDiffStatus::Modified => {
                        if !row.cell_diffs.is_empty() {
                            let set_clauses = row
                                .cell_diffs
                                .iter()
                                .map(|cd| {
                                    format!(
                                        "{} = {}",
                                        quote_ident(&cd.column, report.dialect),
                                        format_query_value_literal(
                                            &cd.source_value,
                                            report.dialect
                                        )
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            let where_clause = build_where_clause(&row.key_values);
                            script.push_str(&format!(
                                "UPDATE {} SET {} WHERE {};\n",
                                quoted_dest_table, set_clauses, where_clause
                            ));
                            update_count += 1;
                        }
                    }
                    DataDiffStatus::Deleted => {
                        let where_clause = build_where_clause(&row.key_values);
                        if options.safe_mode {
                            script.push_str(&format!(
                                "-- [SAFE MODE SKIPPED] DELETE FROM {} WHERE {};\n",
                                quoted_dest_table, where_clause
                            ));
                        } else {
                            script.push_str(&format!(
                                "DELETE FROM {} WHERE {};\n",
                                quoted_dest_table, where_clause
                            ));
                        }
                        delete_count += 1;
                    }
                    DataDiffStatus::Identical => {}
                }
            }
        }
        MigrationDirection::TargetToSource => {
            // TargetToSource:
            // Deleted (exists in target, missing in source) -> INSERT into source
            // Modified -> UPDATE source with target values
            // Added (exists in source, missing in target) -> DELETE from source
            for row in &report.rows {
                match row.status {
                    DataDiffStatus::Deleted => {
                        if let Some(ref t_row) = row.target_row {
                            let values_str = t_row
                                .iter()
                                .map(|v| format_query_value_literal(v, report.dialect))
                                .collect::<Vec<_>>()
                                .join(", ");
                            script.push_str(&format!(
                                "INSERT INTO {} ({}) VALUES ({});\n",
                                quoted_dest_table, col_list, values_str
                            ));
                            insert_count += 1;
                        }
                    }
                    DataDiffStatus::Modified => {
                        if !row.cell_diffs.is_empty() {
                            let set_clauses = row
                                .cell_diffs
                                .iter()
                                .map(|cd| {
                                    format!(
                                        "{} = {}",
                                        quote_ident(&cd.column, report.dialect),
                                        format_query_value_literal(
                                            &cd.target_value,
                                            report.dialect
                                        )
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            let where_clause = build_where_clause(&row.key_values);
                            script.push_str(&format!(
                                "UPDATE {} SET {} WHERE {};\n",
                                quoted_dest_table, set_clauses, where_clause
                            ));
                            update_count += 1;
                        }
                    }
                    DataDiffStatus::Added => {
                        let where_clause = build_where_clause(&row.key_values);
                        if options.safe_mode {
                            script.push_str(&format!(
                                "-- [SAFE MODE SKIPPED] DELETE FROM {} WHERE {};\n",
                                quoted_dest_table, where_clause
                            ));
                        } else {
                            script.push_str(&format!(
                                "DELETE FROM {} WHERE {};\n",
                                quoted_dest_table, where_clause
                            ));
                        }
                        delete_count += 1;
                    }
                    DataDiffStatus::Identical => {}
                }
            }
        }
    }

    if options.wrap_transaction {
        script.push_str("\nCOMMIT;\n");
    }

    let total_statements = insert_count + update_count + delete_count;

    DataSyncScript {
        sql: script,
        insert_count,
        update_count,
        delete_count,
        total_statements,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_table_data_basic() {
        let cols = vec![
            "id".to_string(),
            "name".to_string(),
            "status".to_string(),
            "price".to_string(),
        ];
        let keys = vec!["id".to_string()];

        let src_rows = vec![
            vec![
                QueryValue::Int(1),
                QueryValue::String("Apple".into()),
                QueryValue::String("active".into()),
                QueryValue::Float(1.5),
            ],
            vec![
                QueryValue::Int(2),
                QueryValue::String("Banana".into()),
                QueryValue::String("active".into()),
                QueryValue::Float(2.0),
            ],
            vec![
                QueryValue::Int(3),
                QueryValue::String("Cherry".into()),
                QueryValue::String("out_of_stock".into()),
                QueryValue::Float(5.0),
            ],
        ];

        let tgt_rows = vec![
            vec![
                QueryValue::Int(1),
                QueryValue::String("Apple".into()),
                QueryValue::String("active".into()),
                QueryValue::Float(1.5),
            ],
            vec![
                QueryValue::Int(2),
                QueryValue::String("Banana".into()),
                QueryValue::String("inactive".into()),
                QueryValue::Float(2.5),
            ],
            vec![
                QueryValue::Int(4),
                QueryValue::String("Date".into()),
                QueryValue::String("active".into()),
                QueryValue::Float(3.0),
            ],
        ];

        let report = compare_table_data(
            "products_src",
            "products_tgt",
            &cols,
            &keys,
            &src_rows,
            &tgt_rows,
            DatabaseFamily::Postgres,
        );

        assert_eq!(report.summary.total_source_rows, 3);
        assert_eq!(report.summary.total_target_rows, 3);
        assert_eq!(report.summary.identical_count, 1); // id=1
        assert_eq!(report.summary.modified_count, 1); // id=2
        assert_eq!(report.summary.added_count, 1); // id=3 (in src only)
        assert_eq!(report.summary.deleted_count, 1); // id=4 (in tgt only)
        assert_eq!(report.summary.total_differences(), 3);

        // Check modified row details
        let mod_row = report
            .rows
            .iter()
            .find(|r| r.status == DataDiffStatus::Modified)
            .expect("should have modified row");
        assert_eq!(mod_row.cell_diffs.len(), 2);
        assert_eq!(mod_row.cell_diffs[0].column, "status");
        assert_eq!(mod_row.cell_diffs[0].source_display, "active");
        assert_eq!(mod_row.cell_diffs[0].target_display, "inactive");
        assert_eq!(mod_row.cell_diffs[1].column, "price");
    }

    #[test]
    fn test_generate_data_sync_sql_postgres() {
        let cols = vec!["id".to_string(), "val".to_string()];
        let keys = vec!["id".to_string()];
        let src_rows = vec![
            vec![QueryValue::Int(1), QueryValue::String("new".into())],
            vec![QueryValue::Int(2), QueryValue::String("mod_src".into())],
        ];
        let tgt_rows = vec![
            vec![QueryValue::Int(2), QueryValue::String("mod_tgt".into())],
            vec![QueryValue::Int(3), QueryValue::String("old".into())],
        ];

        let report = compare_table_data(
            "t_src",
            "t_tgt",
            &cols,
            &keys,
            &src_rows,
            &tgt_rows,
            DatabaseFamily::Postgres,
        );

        // Safe mode (default)
        let opts = DataDiffOptions::default();
        let script = generate_data_sync_sql(&report, &opts);

        assert_eq!(script.insert_count, 1);
        assert_eq!(script.update_count, 1);
        assert_eq!(script.delete_count, 1);
        assert!(script.sql.contains("BEGIN;"));
        assert!(script.sql.contains("COMMIT;"));
        assert!(
            script
                .sql
                .contains("INSERT INTO \"t_tgt\" (\"id\", \"val\") VALUES (1, 'new');")
        );
        assert!(
            script
                .sql
                .contains("UPDATE \"t_tgt\" SET \"val\" = 'mod_src' WHERE \"id\" = 2;")
        );
        assert!(
            script
                .sql
                .contains("-- [SAFE MODE SKIPPED] DELETE FROM \"t_tgt\" WHERE \"id\" = 3;")
        );
    }

    #[test]
    fn test_generate_data_sync_sql_mysql_unsafe() {
        let cols = vec!["id".to_string(), "name".to_string()];
        let keys = vec!["id".to_string()];
        let src_rows = vec![vec![QueryValue::Int(1), QueryValue::String("A".into())]];
        let tgt_rows = vec![vec![QueryValue::Int(2), QueryValue::String("B".into())]];

        let report = compare_table_data(
            "users_src",
            "users_tgt",
            &cols,
            &keys,
            &src_rows,
            &tgt_rows,
            DatabaseFamily::MySql,
        );

        let opts = DataDiffOptions {
            direction: MigrationDirection::SourceToTarget,
            safe_mode: false,
            wrap_transaction: true,
            max_rows: 100,
        };
        let script = generate_data_sync_sql(&report, &opts);

        assert!(script.sql.contains("START TRANSACTION;"));
        assert!(
            script
                .sql
                .contains("INSERT INTO `users_tgt` (`id`, `name`) VALUES (1, 'A');")
        );
        assert!(
            script
                .sql
                .contains("DELETE FROM `users_tgt` WHERE `id` = 2;")
        );
        assert!(!script.sql.contains("-- [SAFE MODE SKIPPED]"));
    }

    #[test]
    fn test_direction_target_to_source() {
        let cols = vec!["id".to_string(), "val".to_string()];
        let keys = vec!["id".to_string()];
        let src_rows = vec![vec![
            QueryValue::Int(1),
            QueryValue::String("src_only".into()),
        ]];
        let tgt_rows = vec![vec![
            QueryValue::Int(2),
            QueryValue::String("tgt_only".into()),
        ]];

        let report = compare_table_data(
            "s_tbl",
            "t_tbl",
            &cols,
            &keys,
            &src_rows,
            &tgt_rows,
            DatabaseFamily::Sqlite,
        );

        let opts = DataDiffOptions {
            direction: MigrationDirection::TargetToSource,
            safe_mode: false,
            wrap_transaction: false,
            max_rows: 100,
        };
        let script = generate_data_sync_sql(&report, &opts);

        // Syncing into source:
        // row 2 (in tgt) should be INSERTed into s_tbl
        // row 1 (in src) should be DELETEd from s_tbl
        assert!(
            script
                .sql
                .contains("INSERT INTO \"s_tbl\" (\"id\", \"val\") VALUES (2, 'tgt_only');")
        );
        assert!(
            script
                .sql
                .contains("DELETE FROM \"s_tbl\" WHERE \"id\" = 1;")
        );
        assert!(!script.sql.contains("BEGIN;"));
    }

    #[test]
    fn test_composite_key_and_null_handling() {
        let cols = vec![
            "tenant_id".to_string(),
            "user_id".to_string(),
            "notes".to_string(),
        ];
        let keys = vec!["tenant_id".to_string(), "user_id".to_string()];

        let src_rows = vec![
            vec![QueryValue::Int(10), QueryValue::Int(101), QueryValue::Null],
            vec![
                QueryValue::Int(10),
                QueryValue::Int(102),
                QueryValue::String("new notes".into()),
            ],
        ];

        let tgt_rows = vec![
            vec![
                QueryValue::Int(10),
                QueryValue::Int(101),
                QueryValue::String("old notes".into()),
            ],
            vec![
                QueryValue::Int(10),
                QueryValue::Int(102),
                QueryValue::String("new notes".into()),
            ],
        ];

        let report = compare_table_data(
            "members_src",
            "members_tgt",
            &cols,
            &keys,
            &src_rows,
            &tgt_rows,
            DatabaseFamily::Postgres,
        );

        assert_eq!(report.summary.identical_count, 1); // (10, 102)
        assert_eq!(report.summary.modified_count, 1); // (10, 101)
        assert_eq!(report.summary.added_count, 0);
        assert_eq!(report.summary.deleted_count, 0);

        let opts = DataDiffOptions::default();
        let script = generate_data_sync_sql(&report, &opts);

        assert!(script.sql.contains("UPDATE \"members_tgt\" SET \"notes\" = NULL WHERE \"tenant_id\" = 10 AND \"user_id\" = 101;"));
    }

    #[test]
    fn test_identical_tables_no_sync_statements() {
        let cols = vec!["id".to_string(), "val".to_string()];
        let keys = vec!["id".to_string()];
        let rows = vec![vec![QueryValue::Int(1), QueryValue::String("hello".into())]];

        let report = compare_table_data(
            "t1",
            "t2",
            &cols,
            &keys,
            &rows,
            &rows,
            DatabaseFamily::Postgres,
        );

        assert!(!report.summary.has_differences());
        assert_eq!(report.summary.identical_count, 1);

        let script = generate_data_sync_sql(&report, &DataDiffOptions::default());
        assert_eq!(script.total_statements, 0);
    }
}
