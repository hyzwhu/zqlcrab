//! Cross-database and cross-connection visual Data & Structure Transfer Engine.
//!
//! Facilitates migrating table schemas (DDL) and table records (DML) between
//! different databases and connections (SQLite, MySQL, PostgreSQL) with dialect type
//! conversion, transactional batches, foreign key protection, and real-time execution audit.

use crate::db::export::export_sql_inserts_with_family;
use crate::db::types::{ColumnInfo, DatabaseFamily, QueryResult, QueryValue, quote_ident};
use serde::{Deserialize, Serialize};

/// Transfer scope for an individual table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TransferScope {
    #[default]
    StructureAndData,
    StructureOnly,
    DataOnly,
}

impl TransferScope {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::StructureAndData => "Structure & Data",
            Self::StructureOnly => "Structure Only",
            Self::DataOnly => "Data Only",
        }
    }
}

/// Mapping and configuration for an individual table transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferTableMapping {
    pub source_table: String,
    pub target_table: String,
    pub scope: TransferScope,
    pub selected: bool,
    pub source_row_count: Option<usize>,
}

impl TransferTableMapping {
    pub fn new(source_table: impl Into<String>) -> Self {
        let name = source_table.into();
        Self {
            source_table: name.clone(),
            target_table: name,
            scope: TransferScope::StructureAndData,
            selected: true,
            source_row_count: None,
        }
    }
}

/// Global options governing the transfer execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferOptions {
    /// Whether to drop the target table if it already exists before creating.
    pub drop_target_if_exists: bool,
    /// Whether to create the target table if it does not exist.
    pub create_target_if_not_exists: bool,
    /// Whether to truncate / clear existing rows in the target table before inserting data.
    pub truncate_target_first: bool,
    /// Batch chunk size for multi-row INSERT statements (e.g. 100, 200, 500, 1000).
    pub batch_size: usize,
    /// Whether to temporarily disable foreign key constraint checks during transfer.
    pub disable_foreign_keys: bool,
    /// Whether to wrap each table or batch in transactions where applicable.
    pub wrap_in_transaction: bool,
    /// If false, abort immediately on error; if true, skip failing table/batch and log.
    pub continue_on_error: bool,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            drop_target_if_exists: false,
            create_target_if_not_exists: true,
            truncate_target_first: false,
            batch_size: 500,
            disable_foreign_keys: true,
            wrap_in_transaction: true,
            continue_on_error: false,
        }
    }
}

/// Severity level for transfer execution audit logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferLogLevel {
    Info,
    Ddl,
    Data,
    Success,
    Warn,
    Error,
}

/// An entry in the live transfer audit log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferAuditLog {
    pub timestamp: String,
    pub level: TransferLogLevel,
    pub message: String,
}

impl TransferAuditLog {
    pub fn new(level: TransferLogLevel, message: impl Into<String>) -> Self {
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f").to_string();
        Self {
            timestamp,
            level,
            message: message.into(),
        }
    }
}

/// Real-time progress metrics during active transfer execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferProgress {
    pub current_table_idx: usize,
    pub total_tables: usize,
    pub current_table_name: String,
    pub current_table_transferred_rows: usize,
    pub current_table_total_rows: usize,
    pub total_rows_transferred: usize,
    pub total_errors: usize,
    pub percentage: f32,
    pub message: String,
}

/// Summary statistics upon transfer completion or abortion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferSummary {
    pub tables_completed: usize,
    pub tables_total: usize,
    pub rows_transferred: usize,
    pub errors_count: usize,
    pub duration_ms: u128,
    pub is_success: bool,
    pub aborted_early: bool,
}

/// Translates column data types between database dialects (SQLite, MySQL, PostgreSQL).
pub fn map_column_type(
    from_type: &str,
    from_family: DatabaseFamily,
    to_family: DatabaseFamily,
) -> String {
    if from_family == to_family {
        return from_type.to_string();
    }

    let upper = from_type.trim().to_uppercase();

    match to_family {
        DatabaseFamily::Sqlite => {
            if upper.contains("INT") || upper.contains("SERIAL") {
                "INTEGER".to_string()
            } else if upper.contains("REAL")
                || upper.contains("FLOAT")
                || upper.contains("DOUBLE")
                || upper.contains("DECIMAL")
                || upper.contains("NUMERIC")
            {
                "REAL".to_string()
            } else if upper.contains("BLOB")
                || upper.contains("BYTEA")
                || upper.contains("BINARY")
            {
                "BLOB".to_string()
            } else {
                "TEXT".to_string()
            }
        }
        DatabaseFamily::MySql => {
            if upper.contains("TINYINT(1)") || upper == "BOOLEAN" || upper == "BOOL" {
                "TINYINT(1)".to_string()
            } else if upper.contains("BIGINT") || upper.contains("BIGSERIAL") {
                "BIGINT".to_string()
            } else if upper.contains("SMALLINT") {
                "SMALLINT".to_string()
            } else if upper.contains("INT") || upper.contains("SERIAL") {
                "INT".to_string()
            } else if upper.contains("FLOAT") {
                "FLOAT".to_string()
            } else if upper.contains("DOUBLE") || upper.contains("REAL") {
                "DOUBLE".to_string()
            } else if upper.contains("DECIMAL") || upper.contains("NUMERIC") {
                if upper.contains('(') {
                    upper
                } else {
                    "DECIMAL(12, 4)".to_string()
                }
            } else if upper.contains("DATETIME") || upper.contains("TIMESTAMPTZ") {
                "DATETIME".to_string()
            } else if upper.contains("TIMESTAMP") {
                "TIMESTAMP".to_string()
            } else if upper.contains("DATE") {
                "DATE".to_string()
            } else if upper.contains("JSON") {
                "JSON".to_string()
            } else if upper.contains("BLOB") || upper.contains("BYTEA") {
                "LONGBLOB".to_string()
            } else if upper.contains("VARCHAR") || upper.contains("CHARACTER VARYING") {
                if upper.contains('(') {
                    upper
                } else {
                    "VARCHAR(255)".to_string()
                }
            } else if upper.contains("CHAR") {
                upper
            } else {
                "LONGTEXT".to_string()
            }
        }
        DatabaseFamily::Postgres => {
            if upper.contains("TINYINT(1)") || upper == "BOOLEAN" || upper == "BOOL" {
                "BOOLEAN".to_string()
            } else if upper.contains("BIGINT") || upper.contains("BIGSERIAL") {
                "BIGINT".to_string()
            } else if upper.contains("SMALLINT") {
                "SMALLINT".to_string()
            } else if upper.contains("INT") || upper.contains("SERIAL") {
                "INTEGER".to_string()
            } else if upper.contains("DOUBLE") || upper.contains("REAL") {
                "DOUBLE PRECISION".to_string()
            } else if upper.contains("FLOAT") {
                "REAL".to_string()
            } else if upper.contains("DECIMAL") || upper.contains("NUMERIC") {
                if upper.contains('(') {
                    upper
                } else {
                    "NUMERIC(12, 4)".to_string()
                }
            } else if upper.contains("TIMESTAMPTZ") {
                "TIMESTAMPTZ".to_string()
            } else if upper.contains("TIMESTAMP") || upper.contains("DATETIME") {
                "TIMESTAMP".to_string()
            } else if upper.contains("DATE") {
                "DATE".to_string()
            } else if upper.contains("JSONB") {
                "JSONB".to_string()
            } else if upper.contains("JSON") {
                "JSON".to_string()
            } else if upper.contains("BLOB") || upper.contains("BYTEA") || upper.contains("BINARY") {
                "BYTEA".to_string()
            } else if upper.contains("VARCHAR") || upper.contains("CHARACTER VARYING") {
                if upper.contains('(') {
                    upper
                } else {
                    "VARCHAR(255)".to_string()
                }
            } else if upper.contains("CHAR") {
                upper
            } else {
                "TEXT".to_string()
            }
        }
    }
}

/// Generates a standard dialect-compliant `CREATE TABLE` DDL statement.
pub fn generate_create_table_ddl(
    target_table: &str,
    columns: &[ColumnInfo],
    pks: &[String],
    from_family: DatabaseFamily,
    to_family: DatabaseFamily,
    if_not_exists: bool,
) -> String {
    let quoted_table = quote_ident(target_table, to_family);
    let exists_clause = if if_not_exists { "IF NOT EXISTS " } else { "" };

    let mut defs = Vec::new();

    // In SQLite, if there's a single integer primary key, it can be defined inline as INTEGER PRIMARY KEY
    let is_single_int_pk_sqlite = to_family == DatabaseFamily::Sqlite
        && pks.len() == 1
        && columns
            .iter()
            .any(|c| c.name == pks[0] && (c.data_type.to_lowercase().contains("int") || c.is_auto_increment));

    for col in columns {
        let quoted_col = quote_ident(&col.name, to_family);
        let mapped_type = map_column_type(&col.data_type, from_family, to_family);

        let mut col_def = format!("{quoted_col} {mapped_type}");

        if is_single_int_pk_sqlite && col.name == pks[0] {
            col_def.push_str(" PRIMARY KEY AUTOINCREMENT");
        } else {
            if !col.is_nullable {
                col_def.push_str(" NOT NULL");
            }
            if let Some(ref def_val) = col.default_value {
                col_def.push_str(&format!(" DEFAULT {def_val}"));
            }
        }

        defs.push(col_def);
    }

    if !is_single_int_pk_sqlite && !pks.is_empty() {
        let pk_cols = pks
            .iter()
            .map(|k| quote_ident(k, to_family))
            .collect::<Vec<_>>()
            .join(", ");
        defs.push(format!("PRIMARY KEY ({pk_cols})"));
    }

    format!(
        "CREATE TABLE {exists_clause}{quoted_table} (\n  {}\n);",
        defs.join(",\n  ")
    )
}

/// Generates a `DROP TABLE IF EXISTS` DDL statement.
pub fn generate_drop_table_ddl(target_table: &str, family: DatabaseFamily) -> String {
    let quoted_table = quote_ident(target_table, family);
    match family {
        DatabaseFamily::Postgres => format!("DROP TABLE IF EXISTS {quoted_table} CASCADE;"),
        _ => format!("DROP TABLE IF EXISTS {quoted_table};"),
    }
}

/// Generates a TRUNCATE or DELETE statement to clear target table records.
pub fn generate_truncate_table_ddl(target_table: &str, family: DatabaseFamily) -> String {
    let quoted_table = quote_ident(target_table, family);
    match family {
        DatabaseFamily::Sqlite => format!("DELETE FROM {quoted_table};"),
        DatabaseFamily::Postgres => format!("TRUNCATE TABLE {quoted_table} CASCADE;"),
        DatabaseFamily::MySql => format!("TRUNCATE TABLE {quoted_table};"),
    }
}

/// Returns SQL commands to disable and re-enable foreign key constraint checks.
pub fn generate_foreign_keys_toggle(family: DatabaseFamily) -> (Option<&'static str>, Option<&'static str>) {
    match family {
        DatabaseFamily::Sqlite => (Some("PRAGMA foreign_keys = OFF;"), Some("PRAGMA foreign_keys = ON;")),
        DatabaseFamily::MySql => (
            Some("SET @OLD_FOREIGN_KEY_CHECKS=@@FOREIGN_KEY_CHECKS, FOREIGN_KEY_CHECKS=0;"),
            Some("SET FOREIGN_KEY_CHECKS=@OLD_FOREIGN_KEY_CHECKS;"),
        ),
        DatabaseFamily::Postgres => (Some("SET CONSTRAINTS ALL DEFERRED;"), None),
    }
}

/// Builds batch multi-row INSERT SQL statements for a chunk of records.
pub fn generate_transfer_batch_insert_sql(
    target_table: &str,
    columns: &[String],
    rows: &[Vec<QueryValue>],
    family: DatabaseFamily,
) -> String {
    if rows.is_empty() || columns.is_empty() {
        return String::new();
    }

    let fake_result = QueryResult {
        columns: columns.to_vec(),
        column_types: vec!["TEXT".to_string(); columns.len()],
        rows: rows.to_vec(),
        rows_affected: None,
        execution_time_ms: Some(0),
    };

    export_sql_inserts_with_family(&fake_result, target_table, rows.len(), family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_column_type_same_family() {
        let mapped = map_column_type("VARCHAR(100)", DatabaseFamily::MySql, DatabaseFamily::MySql);
        assert_eq!(mapped, "VARCHAR(100)");
    }

    #[test]
    fn test_map_column_type_to_sqlite() {
        assert_eq!(map_column_type("BIGINT", DatabaseFamily::Postgres, DatabaseFamily::Sqlite), "INTEGER");
        assert_eq!(map_column_type("DOUBLE PRECISION", DatabaseFamily::Postgres, DatabaseFamily::Sqlite), "REAL");
        assert_eq!(map_column_type("BYTEA", DatabaseFamily::Postgres, DatabaseFamily::Sqlite), "BLOB");
        assert_eq!(map_column_type("VARCHAR(255)", DatabaseFamily::MySql, DatabaseFamily::Sqlite), "TEXT");
    }

    #[test]
    fn test_map_column_type_to_mysql() {
        assert_eq!(map_column_type("INTEGER", DatabaseFamily::Sqlite, DatabaseFamily::MySql), "INT");
        assert_eq!(map_column_type("TEXT", DatabaseFamily::Sqlite, DatabaseFamily::MySql), "LONGTEXT");
        assert_eq!(map_column_type("REAL", DatabaseFamily::Sqlite, DatabaseFamily::MySql), "DOUBLE");
        assert_eq!(map_column_type("BOOLEAN", DatabaseFamily::Postgres, DatabaseFamily::MySql), "TINYINT(1)");
    }

    #[test]
    fn test_map_column_type_to_postgres() {
        assert_eq!(map_column_type("INT", DatabaseFamily::MySql, DatabaseFamily::Postgres), "INTEGER");
        assert_eq!(map_column_type("TINYINT(1)", DatabaseFamily::MySql, DatabaseFamily::Postgres), "BOOLEAN");
        assert_eq!(map_column_type("LONGBLOB", DatabaseFamily::MySql, DatabaseFamily::Postgres), "BYTEA");
        assert_eq!(map_column_type("DATETIME", DatabaseFamily::MySql, DatabaseFamily::Postgres), "TIMESTAMP");
    }

    #[test]
    fn test_generate_create_table_ddl_sqlite() {
        let cols = vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                is_nullable: false,
                is_primary_key: true,
                default_value: None,
                is_auto_increment: true,
                description: None,
            },
            ColumnInfo {
                name: "username".to_string(),
                data_type: "VARCHAR(50)".to_string(),
                is_nullable: false,
                is_primary_key: false,
                default_value: None,
                is_auto_increment: false,
                description: None,
            },
        ];
        let pks = vec!["id".to_string()];
        let ddl = generate_create_table_ddl("users", &cols, &pks, DatabaseFamily::MySql, DatabaseFamily::Sqlite, true);
        assert!(ddl.contains("CREATE TABLE IF NOT EXISTS \"users\""));
        assert!(ddl.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(ddl.contains("\"username\" TEXT NOT NULL"));
    }

    #[test]
    fn test_generate_create_table_ddl_postgres() {
        let cols = vec![
            ColumnInfo {
                name: "user_id".to_string(),
                data_type: "INT".to_string(),
                is_nullable: false,
                is_primary_key: true,
                default_value: None,
                is_auto_increment: false,
                description: None,
            },
            ColumnInfo {
                name: "balance".to_string(),
                data_type: "REAL".to_string(),
                is_nullable: true,
                is_primary_key: false,
                default_value: Some("0.0".to_string()),
                is_auto_increment: false,
                description: None,
            },
        ];
        let pks = vec!["user_id".to_string()];
        let ddl = generate_create_table_ddl("accounts", &cols, &pks, DatabaseFamily::Sqlite, DatabaseFamily::Postgres, false);
        assert!(ddl.contains("CREATE TABLE \"accounts\""));
        assert!(ddl.contains("\"user_id\" INTEGER NOT NULL"));
        assert!(ddl.contains("PRIMARY KEY (\"user_id\")"));
        assert!(ddl.contains("\"balance\" DOUBLE PRECISION DEFAULT 0.0"));
    }

    #[test]
    fn test_generate_drop_and_truncate_ddl() {
        assert_eq!(generate_drop_table_ddl("users", DatabaseFamily::Sqlite), "DROP TABLE IF EXISTS \"users\";");
        assert_eq!(generate_drop_table_ddl("users", DatabaseFamily::Postgres), "DROP TABLE IF EXISTS \"users\" CASCADE;");
        assert_eq!(generate_truncate_table_ddl("users", DatabaseFamily::Sqlite), "DELETE FROM \"users\";");
        assert_eq!(generate_truncate_table_ddl("users", DatabaseFamily::MySql), "TRUNCATE TABLE `users`;");
        assert_eq!(generate_truncate_table_ddl("users", DatabaseFamily::Postgres), "TRUNCATE TABLE \"users\" CASCADE;");
    }

    #[test]
    fn test_generate_batch_insert_sql() {
        let cols = vec!["id".to_string(), "name".to_string()];
        let rows = vec![
            vec![QueryValue::Int(1), QueryValue::String("Alice".to_string())],
            vec![QueryValue::Int(2), QueryValue::String("Bob".to_string())],
        ];
        let sql = generate_transfer_batch_insert_sql("users", &cols, &rows, DatabaseFamily::Sqlite);
        assert!(sql.contains("INSERT INTO \"users\" (\"id\", \"name\") VALUES"));
        assert!(sql.contains("(1, 'Alice')"));
        assert!(sql.contains("(2, 'Bob')"));
    }
}
