//! Database Dump Engine for multi-table and full-database SQL backups.
//! Generates dialect-accurate DDL, DML, foreign key checks, and transaction blocks.

use crate::db::export::{ExportScope, SqlDumpOptions, export_sql_inserts_with_family};
use crate::db::types::{DatabaseFamily, QueryResult, quote_ident};
use serde::{Deserialize, Serialize};

/// Target destination for generated database dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DumpDestination {
    #[default]
    File,
    Clipboard,
    QueryConsole,
}

/// Configuration options for generating a database-wide or multi-table dump.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseDumpConfig {
    pub database: String,
    pub tables: Vec<String>,
    pub scope: ExportScope,
    pub options: SqlDumpOptions,
    pub disable_foreign_keys: bool,
    pub family: DatabaseFamily,
    pub destination: DumpDestination,
}

impl Default for DatabaseDumpConfig {
    fn default() -> Self {
        Self {
            database: String::new(),
            tables: Vec::new(),
            scope: ExportScope::SchemaAndData,
            options: SqlDumpOptions::default(),
            disable_foreign_keys: true,
            family: DatabaseFamily::Sqlite,
            destination: DumpDestination::File,
        }
    }
}

/// Table payload used for dump generation.
#[derive(Debug, Clone)]
pub struct TableDumpPayload {
    pub table_name: String,
    pub ddl: Option<String>,
    pub data: Option<QueryResult>,
}

/// Progress notification payload emitted during dump streaming.
#[derive(Debug, Clone)]
pub struct DumpProgress {
    pub current_table_index: usize,
    pub total_tables: usize,
    pub current_table_name: String,
    pub rows_exported_current_table: usize,
    pub total_rows_exported: usize,
    pub is_finished: bool,
}

/// Final summary upon dump completion.
#[derive(Debug, Clone, Default)]
pub struct DumpSummary {
    pub tables_count: usize,
    pub total_rows: usize,
    pub total_bytes: usize,
    pub duration_ms: u128,
}

/// Generates a complete database dump SQL script for the given tables.
pub fn generate_database_dump_sql(
    config: &DatabaseDumpConfig,
    tables: &[TableDumpPayload],
) -> String {
    let mut out = String::with_capacity(1024 * 16);

    // 1. Header Comments
    if config.options.include_comments {
        out.push_str("-- --------------------------------------------------------\n");
        out.push_str("-- zqlcrab Database Dump Wizard\n");
        if !config.database.is_empty() {
            out.push_str(&format!("-- Database: {}\n", config.database));
        }
        out.push_str(&format!("-- Dialect:  {:?}\n", config.family));
        out.push_str(&format!("-- Scope:    {}\n", config.scope.display_name()));
        out.push_str(&format!("-- Tables:   {}\n", tables.len()));
        out.push_str("-- --------------------------------------------------------\n\n");
    }

    // 2. Foreign Key Safety Preamble
    if config.disable_foreign_keys {
        match config.family {
            DatabaseFamily::MySql => {
                out.push_str("/*!40014 SET @OLD_FOREIGN_KEY_CHECKS=@@FOREIGN_KEY_CHECKS, FOREIGN_KEY_CHECKS=0 */;\n");
                out.push_str("/*!40101 SET @OLD_SQL_MODE=@@SQL_MODE, SQL_MODE='NO_AUTO_VALUE_ON_ZERO' */;\n\n");
            }
            DatabaseFamily::Sqlite => {
                out.push_str("PRAGMA foreign_keys = OFF;\n\n");
            }
            DatabaseFamily::Postgres => {
                out.push_str("SET CONSTRAINTS ALL DEFERRED;\n\n");
            }
        }
    }

    // 3. Transaction Preamble
    if config.options.wrap_in_transaction {
        match config.family {
            DatabaseFamily::Sqlite => out.push_str("BEGIN TRANSACTION;\n\n"),
            DatabaseFamily::Postgres => out.push_str("BEGIN;\n\n"),
            DatabaseFamily::MySql => out.push_str("START TRANSACTION;\n\n"),
        }
    }

    // 4. Tables DDL and DML
    for table_payload in tables {
        let table_name = &table_payload.table_name;
        let quoted_name = quote_ident(table_name, config.family);

        if config.options.include_comments {
            out.push_str(&format!(
                "-- --------------------------------------------------------\n-- Table structure and data for table {}\n-- --------------------------------------------------------\n\n",
                quoted_name
            ));
        }

        // Schema DDL
        if config.scope == ExportScope::SchemaAndData || config.scope == ExportScope::SchemaOnly {
            if config.options.drop_table_if_exists {
                let drop_sql = match config.family {
                    DatabaseFamily::Postgres => {
                        format!("DROP TABLE IF EXISTS {quoted_name} CASCADE;\n\n")
                    }
                    _ => format!("DROP TABLE IF EXISTS {quoted_name};\n\n"),
                };
                out.push_str(&drop_sql);
            }

            if let Some(ddl_text) = &table_payload.ddl {
                let trimmed = ddl_text.trim();
                if !trimmed.is_empty() {
                    out.push_str(trimmed);
                    if !trimmed.ends_with(';') {
                        out.push(';');
                    }
                    out.push_str("\n\n");
                }
            }
        }

        // Data DML Inserts
        if (config.scope == ExportScope::SchemaAndData || config.scope == ExportScope::DataOnly)
            && let Some(data) = &table_payload.data
            && !data.rows.is_empty()
        {
            let inserts = export_sql_inserts_with_family(
                data,
                table_name,
                config.options.batch_size,
                config.family,
            );
            out.push_str(&inserts);
            out.push('\n');
        }
    }

    // 5. Transaction Commit
    if config.options.wrap_in_transaction {
        out.push_str("COMMIT;\n\n");
    }

    // 6. Foreign Key Safety Epilogue
    if config.disable_foreign_keys {
        match config.family {
            DatabaseFamily::MySql => {
                out.push_str("/*!40014 SET FOREIGN_KEY_CHECKS=@OLD_FOREIGN_KEY_CHECKS */;\n");
            }
            DatabaseFamily::Sqlite => {
                out.push_str("PRAGMA foreign_keys = ON;\n");
            }
            DatabaseFamily::Postgres => {
                // Constraints automatically checked on COMMIT in Postgres
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::QueryValue;

    #[test]
    fn test_generate_database_dump_sqlite_full() {
        let mut config = DatabaseDumpConfig::default();
        config.database = "test_db.sqlite".to_string();
        config.family = DatabaseFamily::Sqlite;
        config.scope = ExportScope::SchemaAndData;
        config.disable_foreign_keys = true;
        config.options.wrap_in_transaction = true;

        let table1 = TableDumpPayload {
            table_name: "users".to_string(),
            ddl: Some("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);".to_string()),
            data: Some(QueryResult {
                columns: vec!["id".to_string(), "name".to_string()],
                column_types: vec!["INTEGER".to_string(), "TEXT".to_string()],
                rows: vec![
                    vec![QueryValue::Int(1), QueryValue::String("Alice".to_string())],
                    vec![QueryValue::Int(2), QueryValue::String("Bob".to_string())],
                ],
                rows_affected: None,
                execution_time_ms: Some(0),
            }),
        };

        let sql = generate_database_dump_sql(&config, &[table1]);
        assert!(sql.contains("PRAGMA foreign_keys = OFF;"));
        assert!(sql.contains("BEGIN TRANSACTION;"));
        assert!(sql.contains("DROP TABLE IF EXISTS \"users\";"));
        assert!(sql.contains("CREATE TABLE users"));
        assert!(sql.contains("INSERT INTO \"users\""));
        assert!(sql.contains("COMMIT;"));
        assert!(sql.contains("PRAGMA foreign_keys = ON;"));
    }

    #[test]
    fn test_generate_database_dump_mysql_fk_toggles() {
        let mut config = DatabaseDumpConfig::default();
        config.family = DatabaseFamily::MySql;
        config.scope = ExportScope::SchemaOnly;
        config.disable_foreign_keys = true;
        config.options.wrap_in_transaction = false;

        let table = TableDumpPayload {
            table_name: "orders".to_string(),
            ddl: Some("CREATE TABLE `orders` (`id` INT PRIMARY KEY);".to_string()),
            data: None,
        };

        let sql = generate_database_dump_sql(&config, &[table]);
        assert!(sql.contains("SET @OLD_FOREIGN_KEY_CHECKS=@@FOREIGN_KEY_CHECKS, FOREIGN_KEY_CHECKS=0"));
        assert!(sql.contains("DROP TABLE IF EXISTS `orders`;"));
        assert!(!sql.contains("START TRANSACTION;"));
        assert!(sql.contains("SET FOREIGN_KEY_CHECKS=@OLD_FOREIGN_KEY_CHECKS"));
    }

    #[test]
    fn test_generate_database_dump_data_only() {
        let mut config = DatabaseDumpConfig::default();
        config.family = DatabaseFamily::Postgres;
        config.scope = ExportScope::DataOnly;
        config.options.include_comments = false;
        config.disable_foreign_keys = false;
        config.options.wrap_in_transaction = true;

        let table = TableDumpPayload {
            table_name: "items".to_string(),
            ddl: Some("CREATE TABLE items (id INT);".to_string()),
            data: Some(QueryResult {
                columns: vec!["id".to_string()],
                column_types: vec!["INT".to_string()],
                rows: vec![vec![QueryValue::Int(42)]],
                rows_affected: None,
                execution_time_ms: Some(0),
            }),
        };

        let sql = generate_database_dump_sql(&config, &[table]);
        assert!(!sql.contains("CREATE TABLE"));
        assert!(!sql.contains("DROP TABLE"));
        assert!(sql.contains("BEGIN;"));
        assert!(sql.contains("INSERT INTO \"items\""));
        assert!(sql.contains("COMMIT;"));
    }
}
