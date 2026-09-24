//! Batch chunk insertion executor and SQL script import engine with error isolation.

use crate::db::error::{DbError, DbResult};
use crate::db::handle::ActiveConnection;
use crate::db::import::column_mapper::ColumnMapping;
use crate::db::import::csv_sniffer::{CsvDelimiter, CsvSniffer, FileEncoding};
use crate::db::types::{ColumnInfo, quote_ident};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Error handling policy when an invalid row or insert failure is encountered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ErrorPolicy {
    #[default]
    Skip,
    Abort,
}

impl ErrorPolicy {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Skip => "Skip error rows & isolate to log",
            Self::Abort => "Abort transaction on first error",
        }
    }
}

/// An isolated error row with position, raw content, and failure reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportErrorRow {
    pub line_number: usize,
    pub raw_data: String,
    pub reason: String,
}

/// Real-time progress snapshot of an ongoing import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub processed_rows: usize,
    pub succeeded_rows: usize,
    pub failed_rows: usize,
    pub total_estimated_rows: usize,
    pub throughput_rows_per_sec: f64,
    pub elapsed_secs: f64,
}

/// Final summary result of an import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub total_processed: usize,
    pub total_succeeded: usize,
    pub total_failed: usize,
    pub elapsed_millis: u64,
    pub error_rows: Vec<ImportErrorRow>,
}

/// Configuration parameters for importing a CSV or TSV file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvImportConfig {
    pub target_table: String,
    pub mappings: Vec<ColumnMapping>,
    pub delimiter: CsvDelimiter,
    pub encoding: FileEncoding,
    pub has_headers: bool,
    pub batch_size: usize,
    pub error_policy: ErrorPolicy,
}

/// Formats a raw CSV cell text into a safely quoted and typed SQL literal.
pub fn format_csv_value_for_sql(raw: &str, col_type: Option<&str>) -> String {
    let trimmed = raw.trim();

    // 1. Explicit or empty nulls
    if trimmed.eq_ignore_ascii_case("null") || trimmed.eq_ignore_ascii_case("\\n") {
        return "NULL".to_string();
    }

    if let Some(col_type) = col_type {
        let type_lower = col_type.to_lowercase();
        if trimmed.is_empty() && !type_lower.contains("char") && !type_lower.contains("text") {
            return "NULL".to_string();
        }

        if type_lower.contains("bool") {
            if trimmed == "1"
                || trimmed.eq_ignore_ascii_case("true")
                || trimmed.eq_ignore_ascii_case("t")
            {
                return "TRUE".to_string();
            } else if trimmed == "0"
                || trimmed.eq_ignore_ascii_case("false")
                || trimmed.eq_ignore_ascii_case("f")
            {
                return "FALSE".to_string();
            }
        }

        if type_lower.contains("int")
            || type_lower.contains("serial")
            || type_lower.contains("numeric")
            || type_lower.contains("decimal")
            || type_lower.contains("float")
            || type_lower.contains("double")
            || type_lower.contains("real")
        {
            if let Ok(num) = trimmed.parse::<f64>() {
                if num.is_finite() {
                    return trimmed.to_string();
                }
            }
        }
    } else if trimmed.is_empty() {
        return "NULL".to_string();
    }

    // Default string literal: escape single quotes by doubling them
    let escaped = raw.replace('\'', "''");
    format!("'{escaped}'")
}

pub struct ImportExecutor;

impl ImportExecutor {
    /// Executes a CSV/TSV batch import into the active database connection.
    pub async fn execute_csv_import(
        conn: &ActiveConnection,
        path: &Path,
        config: &CsvImportConfig,
        table_columns: &[ColumnInfo],
        on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
    ) -> DbResult<ImportResult> {
        let start_time = Instant::now();
        let family = conn.config.db_type.family();

        // Validate target columns from mappings
        let active_mappings: Vec<(usize, String, Option<String>)> = config
            .mappings
            .iter()
            .filter_map(|m| {
                m.target_column.as_ref().map(|target_col| {
                    let col_type = table_columns
                        .iter()
                        .find(|c| &c.name == target_col)
                        .map(|c| c.data_type.clone());
                    (m.source_index, target_col.clone(), col_type)
                })
            })
            .collect();

        if active_mappings.is_empty() {
            return Err(DbError::Configuration(
                "At least one source column must be mapped to a target table column".to_string(),
            ));
        }

        // Open and read file using detected encoding
        let file_bytes = std::fs::read(path).map_err(|e| DbError::Io(e.to_string()))?;
        let decoded_text = CsvSniffer::decode_bytes(&file_bytes, config.encoding);

        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(config.delimiter.as_byte())
            .has_headers(config.has_headers)
            .flexible(true)
            .from_reader(decoded_text.as_bytes());

        let target_col_names: Vec<String> = active_mappings
            .iter()
            .map(|(_, col, _)| quote_ident(col, family))
            .collect();
        let cols_sql = target_col_names.join(", ");
        let table_sql = quote_ident(&config.target_table, family);

        let batch_size = config.batch_size.clamp(10, 5000);
        let mut chunk_rows: Vec<(usize, String, String)> = Vec::with_capacity(batch_size); // (line_number, raw_data, sql_value_tuple)

        let mut total_processed = 0;
        let mut total_succeeded = 0;
        let mut total_failed = 0;
        let mut error_rows = Vec::new();
        let mut current_line = if config.has_headers { 2 } else { 1 };

        let total_estimated = decoded_text
            .lines()
            .count()
            .saturating_sub(if config.has_headers { 1 } else { 0 });

        for result in rdr.records() {
            match result {
                Ok(record) => {
                    total_processed += 1;
                    let raw_str = record
                        .iter()
                        .collect::<Vec<_>>()
                        .join(match config.delimiter {
                            CsvDelimiter::Tab => "\t",
                            CsvDelimiter::Semicolon => ";",
                            CsvDelimiter::Pipe => "|",
                            _ => ",",
                        });

                    // Build value tuple
                    let mut vals = Vec::with_capacity(active_mappings.len());
                    for (src_idx, _, col_type) in &active_mappings {
                        let raw_val = record.get(*src_idx).unwrap_or("");
                        vals.push(format_csv_value_for_sql(raw_val, col_type.as_deref()));
                    }
                    let tuple_sql = format!("({})", vals.join(", "));
                    chunk_rows.push((current_line, raw_str, tuple_sql));
                }
                Err(err) => {
                    total_processed += 1;
                    total_failed += 1;
                    let err_msg = format!("CSV parse error: {err}");
                    if config.error_policy == ErrorPolicy::Abort {
                        return Err(DbError::QueryExecution(err_msg));
                    }
                    error_rows.push(ImportErrorRow {
                        line_number: current_line,
                        raw_data: "<malformed line>".to_string(),
                        reason: err_msg,
                    });
                }
            }
            current_line += 1;

            // Execute batch when chunk is full
            if chunk_rows.len() >= batch_size {
                Self::flush_chunk(
                    conn,
                    &table_sql,
                    &cols_sql,
                    &mut chunk_rows,
                    config.error_policy,
                    &mut total_succeeded,
                    &mut total_failed,
                    &mut error_rows,
                )
                .await?;

                if let Some(ref cb) = on_progress {
                    let elapsed = start_time.elapsed().as_secs_f64().max(0.001);
                    cb(ImportProgress {
                        processed_rows: total_processed,
                        succeeded_rows: total_succeeded,
                        failed_rows: total_failed,
                        total_estimated_rows: total_estimated,
                        throughput_rows_per_sec: (total_processed as f64 / elapsed).round(),
                        elapsed_secs: elapsed,
                    });
                }
            }
        }

        // Flush remaining rows
        if !chunk_rows.is_empty() {
            Self::flush_chunk(
                conn,
                &table_sql,
                &cols_sql,
                &mut chunk_rows,
                config.error_policy,
                &mut total_succeeded,
                &mut total_failed,
                &mut error_rows,
            )
            .await?;
        }

        let elapsed_millis = start_time.elapsed().as_millis() as u64;

        if let Some(ref cb) = on_progress {
            let elapsed_secs = start_time.elapsed().as_secs_f64().max(0.001);
            cb(ImportProgress {
                processed_rows: total_processed,
                succeeded_rows: total_succeeded,
                failed_rows: total_failed,
                total_estimated_rows: total_estimated,
                throughput_rows_per_sec: (total_processed as f64 / elapsed_secs).round(),
                elapsed_secs,
            });
        }

        Ok(ImportResult {
            total_processed,
            total_succeeded,
            total_failed,
            elapsed_millis,
            error_rows,
        })
    }

    /// Flushes a batch of rows to the database, falling back to row-by-row isolation on failure if ErrorPolicy::Skip.
    async fn flush_chunk(
        conn: &ActiveConnection,
        table_sql: &str,
        cols_sql: &str,
        chunk_rows: &mut Vec<(usize, String, String)>,
        error_policy: ErrorPolicy,
        total_succeeded: &mut usize,
        total_failed: &mut usize,
        error_rows: &mut Vec<ImportErrorRow>,
    ) -> DbResult<()> {
        if chunk_rows.is_empty() {
            return Ok(());
        }

        let tuples: Vec<&str> = chunk_rows.iter().map(|(_, _, t)| t.as_str()).collect();
        let batch_sql = format!(
            "INSERT INTO {table_sql} ({cols_sql}) VALUES {};",
            tuples.join(",\n")
        );

        match conn.execute_batch(&batch_sql).await {
            Ok(_) => {
                *total_succeeded += chunk_rows.len();
                chunk_rows.clear();
                Ok(())
            }
            Err(err) => {
                if error_policy == ErrorPolicy::Abort {
                    return Err(err);
                }

                // Fallback: isolate individual rows in chunk to preserve valid records
                for (line_no, raw_data, tuple) in chunk_rows.drain(..) {
                    let single_sql =
                        format!("INSERT INTO {table_sql} ({cols_sql}) VALUES {tuple};");
                    match conn.execute_batch(&single_sql).await {
                        Ok(_) => {
                            *total_succeeded += 1;
                        }
                        Err(single_err) => {
                            *total_failed += 1;
                            error_rows.push(ImportErrorRow {
                                line_number: line_no,
                                raw_data,
                                reason: single_err.to_string(),
                            });
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// Executes an external SQL script file with statement batching and progress updates.
    pub async fn execute_sql_import(
        conn: &ActiveConnection,
        path: &Path,
        error_policy: ErrorPolicy,
        on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
    ) -> DbResult<ImportResult> {
        let start_time = Instant::now();
        let file = File::open(path).map_err(|e| DbError::Io(e.to_string()))?;
        let reader = BufReader::new(file);

        let mut statements = Vec::new();
        let mut current_stmt = String::new();

        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.starts_with("--") || trimmed.starts_with("/*") {
                continue;
            }
            if !current_stmt.is_empty() {
                current_stmt.push('\n');
            }
            current_stmt.push_str(&line);

            if trimmed.ends_with(';') {
                statements.push(current_stmt.trim().to_string());
                current_stmt.clear();
            }
        }

        if !current_stmt.trim().is_empty() {
            statements.push(current_stmt.trim().to_string());
        }

        let total_statements = statements.len();
        let mut total_succeeded = 0;
        let mut total_failed = 0;
        let mut error_rows = Vec::new();

        for (idx, stmt) in statements.into_iter().enumerate() {
            match conn.execute_batch(&stmt).await {
                Ok(_) => {
                    total_succeeded += 1;
                }
                Err(err) => {
                    total_failed += 1;
                    if error_policy == ErrorPolicy::Abort {
                        return Err(err);
                    }
                    error_rows.push(ImportErrorRow {
                        line_number: idx + 1,
                        raw_data: stmt.chars().take(120).collect(),
                        reason: err.to_string(),
                    });
                }
            }

            if let Some(ref cb) = on_progress {
                let elapsed = start_time.elapsed().as_secs_f64().max(0.001);
                cb(ImportProgress {
                    processed_rows: idx + 1,
                    succeeded_rows: total_succeeded,
                    failed_rows: total_failed,
                    total_estimated_rows: total_statements,
                    throughput_rows_per_sec: ((idx + 1) as f64 / elapsed).round(),
                    elapsed_secs: elapsed,
                });
            }
        }

        let elapsed_millis = start_time.elapsed().as_millis() as u64;

        Ok(ImportResult {
            total_processed: total_statements,
            total_succeeded,
            total_failed,
            elapsed_millis,
            error_rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::handle::ActiveConnection;
    use crate::db::import::column_mapper::ColumnMapping;
    use crate::db::import::csv_sniffer::{CsvDelimiter, FileEncoding};
    use crate::db::types::{ColumnInfo, ConnectionConfig};
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_format_cell_value() {
        assert_eq!(
            format_csv_value_for_sql("hello 'world'", Some("TEXT")),
            "'hello ''world'''"
        );
        assert_eq!(format_csv_value_for_sql("NULL", Some("INTEGER")), "NULL");
        assert_eq!(format_csv_value_for_sql("42", Some("INTEGER")), "42");
        assert_eq!(format_csv_value_for_sql("true", Some("BOOLEAN")), "TRUE");
    }

    #[tokio::test]
    async fn test_csv_batch_import_and_isolation_sqlite() {
        let conn_config = ConnectionConfig::sqlite("test_sqlite_import", ":memory:");
        let conn = ActiveConnection::connect_config(conn_config).await.unwrap();

        // Create target table
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL, age INT);",
        )
        .await
        .unwrap();

        let table_cols = vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "username".to_string(),
                data_type: "TEXT".to_string(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "age".to_string(),
                data_type: "INT".to_string(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];

        // Prepare test CSV with 4 rows: row 1-3 valid, row 4 has duplicate id=1 (causes primary key collision)
        let temp_csv = std::env::temp_dir().join("test_executor_import.csv");
        {
            let mut f = std::fs::File::create(&temp_csv).unwrap();
            writeln!(f, "user_id,name,user_age").unwrap();
            writeln!(f, "1,Alice,30").unwrap();
            writeln!(f, "2,Bob,25").unwrap();
            writeln!(f, "3,Charlie,40").unwrap();
            writeln!(f, "1,DuplicateAlice,99").unwrap();
        }

        let config = CsvImportConfig {
            target_table: "users".to_string(),
            delimiter: CsvDelimiter::Comma,
            encoding: FileEncoding::Utf8,
            has_headers: true,
            batch_size: 2, // Forces chunking across 2 batches
            error_policy: ErrorPolicy::Skip,
            mappings: vec![
                ColumnMapping {
                    source_index: 0,
                    source_header: "user_id".to_string(),
                    target_column: Some("id".to_string()),
                },
                ColumnMapping {
                    source_index: 1,
                    source_header: "name".to_string(),
                    target_column: Some("username".to_string()),
                },
                ColumnMapping {
                    source_index: 2,
                    source_header: "user_age".to_string(),
                    target_column: Some("age".to_string()),
                },
            ],
        };

        let progress_calls = Arc::new(AtomicUsize::new(0));
        let progress_calls_clone = progress_calls.clone();

        let res = ImportExecutor::execute_csv_import(
            &conn,
            &temp_csv,
            &config,
            &table_cols,
            Some(Arc::new(move |_| {
                progress_calls_clone.fetch_add(1, Ordering::SeqCst);
            })),
        )
        .await
        .unwrap();

        assert_eq!(res.total_processed, 4);
        assert_eq!(res.total_succeeded, 3);
        assert_eq!(res.total_failed, 1);
        assert_eq!(res.error_rows.len(), 1);
        assert_eq!(res.error_rows[0].line_number, 5); // 1-based line: header=1, row4=5
        assert!(progress_calls.load(Ordering::SeqCst) > 0);

        // Verify rows inserted in SQLite
        let q = conn
            .execute_query("SELECT COUNT(*) AS n FROM users;")
            .await
            .unwrap();
        assert_eq!(q.rows[0][0], crate::db::types::QueryValue::Int(3));

        let _ = std::fs::remove_file(&temp_csv);
    }

    #[tokio::test]
    async fn test_sql_import_sqlite() {
        let conn_config = ConnectionConfig::sqlite("test_sql_import", ":memory:");
        let conn = ActiveConnection::connect_config(conn_config).await.unwrap();

        let temp_sql = std::env::temp_dir().join("test_executor_import.sql");
        {
            let mut f = std::fs::File::create(&temp_sql).unwrap();
            writeln!(f, "CREATE TABLE items (id INT, title TEXT);").unwrap();
            writeln!(f, "INSERT INTO items VALUES (1, 'Book');").unwrap();
            writeln!(f, "INSERT INTO items VALUES (2, 'Pen');").unwrap();
        }

        let res = ImportExecutor::execute_sql_import(&conn, &temp_sql, ErrorPolicy::Abort, None)
            .await
            .unwrap();

        assert_eq!(res.total_processed, 3);
        assert_eq!(res.total_succeeded, 3);
        assert_eq!(res.total_failed, 0);

        let q = conn
            .execute_query("SELECT COUNT(*) AS n FROM items;")
            .await
            .unwrap();
        assert_eq!(q.rows[0][0], crate::db::types::QueryValue::Int(2));

        let _ = std::fs::remove_file(&temp_sql);
    }
}
