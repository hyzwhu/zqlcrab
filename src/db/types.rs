//! Type definitions for database metadata, connections, and query results.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported database types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Sqlite,
    Postgres,
    Mysql,
}

impl DatabaseType {
    /// Default network port for the database system.
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Sqlite => 0,
            Self::Postgres => 5432,
            Self::Mysql => 3306,
        }
    }

    /// User-facing display title.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Sqlite => "SQLite",
            Self::Postgres => "PostgreSQL",
            Self::Mysql => "MySQL",
        }
    }

    /// Default file extension or descriptor.
    pub fn default_database(&self) -> &'static str {
        match self {
            Self::Sqlite => "main.db",
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
        }
    }
}

impl fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Connection profile for establishing a database connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Unique identifier for this connection config.
    pub id: String,
    /// User-given profile name (e.g. "Local SQLite", "Production PG").
    pub name: String,
    /// Database engine type.
    pub db_type: DatabaseType,
    /// Host or IP address (for networked databases).
    #[serde(default = "default_host")]
    pub host: String,
    /// Port number.
    pub port: u16,
    /// Database name or file path for SQLite.
    pub database: String,
    /// Authentication username.
    #[serde(default)]
    pub username: String,
    /// Authentication password.
    #[serde(default)]
    pub password: Option<String>,
    /// Connection timeout in seconds.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    /// Query execution timeout in seconds.
    #[serde(default = "default_query_timeout")]
    pub query_timeout_secs: u64,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_connect_timeout() -> u64 {
    10
}

fn default_query_timeout() -> u64 {
    30
}

impl ConnectionConfig {
    /// Create a SQLite connection profile.
    pub fn sqlite(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            db_type: DatabaseType::Sqlite,
            host: String::new(),
            port: 0,
            database: path.into(),
            username: String::new(),
            password: None,
            connect_timeout_secs: 5,
            query_timeout_secs: 30,
        }
    }

    /// Create a PostgreSQL connection profile.
    pub fn postgres(
        name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
        username: impl Into<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            db_type: DatabaseType::Postgres,
            host: host.into(),
            port,
            database: database.into(),
            username: username.into(),
            password,
            connect_timeout_secs: 10,
            query_timeout_secs: 30,
        }
    }

    /// Create a MySQL connection profile.
    pub fn mysql(
        name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
        username: impl Into<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            db_type: DatabaseType::Mysql,
            host: host.into(),
            port,
            database: database.into(),
            username: username.into(),
            password,
            connect_timeout_secs: 10,
            query_timeout_secs: 30,
        }
    }
}

/// Status and metadata of an active database connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub server_version: Option<String>,
    pub current_database: Option<String>,
    pub ping_ms: Option<u64>,
}

/// A dynamic SQL scalar or cell value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    DateTime(String),
}

impl QueryValue {
    /// Formats the query value as a displayable string for UI grids.
    pub fn to_display_string(&self) -> String {
        match self {
            Self::Null => "NULL".to_string(),
            Self::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => format!("{:.4}", f).trim_end_matches('0').trim_end_matches('.').to_string(),
            Self::String(s) => s.clone(),
            Self::Bytes(b) => format!("<blob: {} bytes>", b.len()),
            Self::DateTime(d) => d.clone(),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl fmt::Display for QueryValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

/// Complete tabular results returned from query execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryResult {
    /// Ordered column names.
    pub columns: Vec<String>,
    /// Column data types reported by the driver.
    pub column_types: Vec<String>,
    /// Table rows where each row contains values in the order of `columns`.
    pub rows: Vec<Vec<QueryValue>>,
    /// Number of rows affected (for DML / DDL operations).
    pub rows_affected: Option<u64>,
    /// Execution time in milliseconds.
    pub execution_time_ms: Option<u64>,
}

impl QueryResult {
    /// Constructs a result for row queries (SELECT, PRAGMA, EXPLAIN, etc.).
    pub fn rows(columns: Vec<String>, column_types: Vec<String>, rows: Vec<Vec<QueryValue>>) -> Self {
        Self {
            columns,
            column_types,
            rows,
            rows_affected: None,
            execution_time_ms: None,
        }
    }

    /// Constructs a result for mutation queries (INSERT, UPDATE, DELETE, etc.).
    pub fn affected(rows_affected: u64) -> Self {
        Self {
            columns: Vec::new(),
            column_types: Vec::new(),
            rows: Vec::new(),
            rows_affected: Some(rows_affected),
            execution_time_ms: None,
        }
    }

    /// Total rows in this result set.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

/// Metadata describing a single table column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub is_auto_increment: bool,
    pub default_value: Option<String>,
    pub description: Option<String>,
}

/// Metadata describing a table or view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub schema: Option<String>,
    pub table_type: String, // "TABLE" or "VIEW"
    pub comment: Option<String>,
    pub row_count_estimate: Option<u64>,
}

/// Database schema entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub name: String,
    pub is_system: bool,
}

/// Table index information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
}
