//! Type definitions for database metadata, connections, and query results.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Underlying engine driver family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseFamily {
    Sqlite,
    Postgres,
    MySql,
}

/// Categorization of database systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseCategory {
    Relational,
    Analytical,
    Distributed,
    Embedded,
}

impl DatabaseCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Relational => "Relational (OLTP)",
            Self::Analytical => "Analytical (OLAP)",
            Self::Distributed => "Distributed SQL",
            Self::Embedded => "Embedded / Local",
        }
    }
}

/// Supported database types including wire-compatible ecosystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    // Embedded
    Sqlite,

    // MySQL Protocol Ecosystem
    Mysql,
    MariaDB,
    TiDB,
    OceanBase,
    StarRocks,
    Doris,
    PolarDB,
    TDSQL,
    SelectDB,
    Databend,
    GoldenDB,
    SingleStore,
    ManticoreSearch,
    CloudSQLMySQL,

    // PostgreSQL Protocol Ecosystem
    Postgres,
    CockroachDB,
    TimescaleDB,
    Redshift,
    YugabyteDB,
    OpenGauss,
    Kingbase,
    GaussDB,
    Greenplum,
    QuestDB,
    Vastbase,
    YashanDB,
    HighGo,
    UXDB,
    GBase8c,
    EnterpriseDB,
    CrateDB,
    Materialize,
    AlloyDB,
    CloudSQLPG,
    FujitsuPG,
}

impl DatabaseType {
    /// Underlying protocol driver family.
    pub fn family(&self) -> DatabaseFamily {
        match self {
            Self::Sqlite => DatabaseFamily::Sqlite,

            Self::Mysql
            | Self::MariaDB
            | Self::TiDB
            | Self::OceanBase
            | Self::StarRocks
            | Self::Doris
            | Self::PolarDB
            | Self::TDSQL
            | Self::SelectDB
            | Self::Databend
            | Self::GoldenDB
            | Self::SingleStore
            | Self::ManticoreSearch
            | Self::CloudSQLMySQL => DatabaseFamily::MySql,

            Self::Postgres
            | Self::CockroachDB
            | Self::TimescaleDB
            | Self::Redshift
            | Self::YugabyteDB
            | Self::OpenGauss
            | Self::Kingbase
            | Self::GaussDB
            | Self::Greenplum
            | Self::QuestDB
            | Self::Vastbase
            | Self::YashanDB
            | Self::HighGo
            | Self::UXDB
            | Self::GBase8c
            | Self::EnterpriseDB
            | Self::CrateDB
            | Self::Materialize
            | Self::AlloyDB
            | Self::CloudSQLPG
            | Self::FujitsuPG => DatabaseFamily::Postgres,
        }
    }

    /// System category.
    pub fn category(&self) -> DatabaseCategory {
        match self {
            Self::Sqlite => DatabaseCategory::Embedded,

            Self::Mysql
            | Self::MariaDB
            | Self::PolarDB
            | Self::CloudSQLMySQL
            | Self::Postgres
            | Self::OpenGauss
            | Self::Kingbase
            | Self::Vastbase
            | Self::YashanDB
            | Self::HighGo
            | Self::UXDB
            | Self::EnterpriseDB
            | Self::AlloyDB
            | Self::CloudSQLPG
            | Self::FujitsuPG => DatabaseCategory::Relational,

            Self::StarRocks
            | Self::Doris
            | Self::SelectDB
            | Self::Databend
            | Self::SingleStore
            | Self::ManticoreSearch
            | Self::TimescaleDB
            | Self::Redshift
            | Self::Greenplum
            | Self::QuestDB
            | Self::Materialize => DatabaseCategory::Analytical,

            Self::TiDB
            | Self::OceanBase
            | Self::TDSQL
            | Self::GoldenDB
            | Self::CockroachDB
            | Self::YugabyteDB
            | Self::GaussDB
            | Self::GBase8c
            | Self::CrateDB => DatabaseCategory::Distributed,
        }
    }

    /// Default network port for the database system.
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Sqlite => 0,
            Self::Mysql
            | Self::MariaDB
            | Self::PolarDB
            | Self::TDSQL
            | Self::GoldenDB
            | Self::SingleStore
            | Self::CloudSQLMySQL => 3306,
            Self::TiDB => 4000,
            Self::OceanBase => 2881,
            Self::StarRocks | Self::Doris | Self::SelectDB => 9030,
            Self::Databend => 3307,
            Self::ManticoreSearch => 9306,
            Self::Postgres
            | Self::TimescaleDB
            | Self::Greenplum
            | Self::OpenGauss
            | Self::GaussDB
            | Self::Vastbase
            | Self::UXDB
            | Self::GBase8c
            | Self::CrateDB
            | Self::AlloyDB
            | Self::CloudSQLPG
            | Self::FujitsuPG => 5432,
            Self::HighGo => 5866,
            Self::CockroachDB => 26257,
            Self::Redshift => 5439,
            Self::YugabyteDB => 5433,
            Self::Kingbase => 54321,
            Self::QuestDB => 8812,
            Self::YashanDB => 1688,
            Self::EnterpriseDB => 5444,
            Self::Materialize => 6875,
        }
    }

    /// User-facing display title.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Sqlite => "SQLite",
            Self::Mysql => "MySQL",
            Self::MariaDB => "MariaDB",
            Self::TiDB => "TiDB",
            Self::OceanBase => "OceanBase",
            Self::StarRocks => "StarRocks",
            Self::Doris => "Apache Doris",
            Self::PolarDB => "PolarDB",
            Self::TDSQL => "Tencent TDSQL",
            Self::SelectDB => "SelectDB",
            Self::Databend => "Databend",
            Self::GoldenDB => "GoldenDB",
            Self::SingleStore => "SingleStore",
            Self::ManticoreSearch => "Manticore Search",
            Self::CloudSQLMySQL => "Cloud SQL (MySQL)",
            Self::Postgres => "PostgreSQL",
            Self::CockroachDB => "CockroachDB",
            Self::TimescaleDB => "TimescaleDB",
            Self::Redshift => "Amazon Redshift",
            Self::YugabyteDB => "YugabyteDB",
            Self::OpenGauss => "openGauss",
            Self::Kingbase => "KingbaseES",
            Self::GaussDB => "GaussDB",
            Self::Greenplum => "Greenplum",
            Self::QuestDB => "QuestDB",
            Self::Vastbase => "Vastbase",
            Self::YashanDB => "YashanDB",
            Self::HighGo => "HighGo",
            Self::UXDB => "UXDB",
            Self::GBase8c => "GBase 8c",
            Self::EnterpriseDB => "EnterpriseDB",
            Self::CrateDB => "CrateDB",
            Self::Materialize => "Materialize",
            Self::AlloyDB => "Google Cloud AlloyDB",
            Self::CloudSQLPG => "Cloud SQL (PG)",
            Self::FujitsuPG => "Fujitsu Enterprise Postgres",
        }
    }

    /// Default database name.
    pub fn default_database(&self) -> &'static str {
        match self {
            Self::Sqlite => ":memory:",
            Self::TiDB | Self::OceanBase => "test",
            Self::StarRocks => "default_cluster",
            Self::Doris | Self::SelectDB | Self::SingleStore => "information_schema",
            Self::Databend => "default",
            Self::Mysql
            | Self::MariaDB
            | Self::PolarDB
            | Self::TDSQL
            | Self::GoldenDB
            | Self::ManticoreSearch
            | Self::CloudSQLMySQL => "mysql",

            Self::CockroachDB => "defaultdb",
            Self::Redshift => "dev",
            Self::YugabyteDB => "yugabyte",
            Self::Kingbase => "kingbase",
            Self::QuestDB => "qdb",
            Self::Vastbase => "vastbase",
            Self::YashanDB => "yashan",
            Self::HighGo => "highgo",
            Self::UXDB => "uxdb",
            Self::EnterpriseDB => "edb",
            Self::CrateDB => "crate",
            Self::Materialize => "materialize",
            Self::Postgres
            | Self::TimescaleDB
            | Self::OpenGauss
            | Self::GaussDB
            | Self::Greenplum
            | Self::GBase8c
            | Self::AlloyDB
            | Self::CloudSQLPG
            | Self::FujitsuPG => "postgres",
        }
    }

    /// Default username.
    pub fn default_user(&self) -> &'static str {
        match self {
            Self::Sqlite => "",
            Self::Mysql
            | Self::MariaDB
            | Self::TiDB
            | Self::OceanBase
            | Self::StarRocks
            | Self::Doris
            | Self::PolarDB
            | Self::TDSQL
            | Self::SelectDB
            | Self::Databend
            | Self::GoldenDB
            | Self::SingleStore
            | Self::ManticoreSearch
            | Self::CloudSQLMySQL => "root",

            Self::Postgres
            | Self::TimescaleDB
            | Self::AlloyDB
            | Self::CloudSQLPG
            | Self::FujitsuPG => "postgres",
            Self::CockroachDB => "root",
            Self::Redshift => "awsuser",
            Self::YugabyteDB => "yugabyte",
            Self::OpenGauss => "omm",
            Self::Kingbase => "system",
            Self::GaussDB => "gaussdb",
            Self::Greenplum => "gpadmin",
            Self::QuestDB => "admin",
            Self::Vastbase => "vastbase",
            Self::YashanDB => "sys",
            Self::HighGo => "highgo",
            Self::UXDB => "uxdb",
            Self::GBase8c => "gbase",
            Self::EnterpriseDB => "enterprisedb",
            Self::CrateDB => "crate",
            Self::Materialize => "materialize",
        }
    }

    /// Whether this is a local file-based database.
    pub fn is_file_based(&self) -> bool {
        matches!(self, Self::Sqlite)
    }

    /// All supported databases.
    pub fn all() -> &'static [DatabaseType] {
        &[
            // Embedded
            Self::Sqlite,
            // MySQL Ecosystem
            Self::Mysql,
            Self::MariaDB,
            Self::TiDB,
            Self::OceanBase,
            Self::StarRocks,
            Self::Doris,
            Self::PolarDB,
            Self::TDSQL,
            Self::SelectDB,
            Self::Databend,
            Self::GoldenDB,
            Self::SingleStore,
            Self::ManticoreSearch,
            Self::CloudSQLMySQL,
            // PostgreSQL Ecosystem
            Self::Postgres,
            Self::CockroachDB,
            Self::TimescaleDB,
            Self::Redshift,
            Self::YugabyteDB,
            Self::OpenGauss,
            Self::Kingbase,
            Self::GaussDB,
            Self::Greenplum,
            Self::QuestDB,
            Self::Vastbase,
            Self::YashanDB,
            Self::HighGo,
            Self::UXDB,
            Self::GBase8c,
            Self::EnterpriseDB,
            Self::CrateDB,
            Self::Materialize,
            Self::AlloyDB,
            Self::CloudSQLPG,
            Self::FujitsuPG,
        ]
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
    /// Whether this connection is in Read-Only mode (destructive queries blocked).
    #[serde(default)]
    pub is_read_only: bool,
    /// Environment tag (e.g. Dev, Test, Prod).
    #[serde(default)]
    pub environment: EnvironmentTag,
    /// SSL/TLS mode.
    #[serde(default)]
    pub ssl_mode: SslMode,
    /// Optional SSH Bastion tunnel configuration.
    #[serde(default)]
    pub ssh_tunnel: Option<SshTunnelConfig>,
    /// Connection pool max connections.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Connection pool min connections.
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
}

fn default_max_connections() -> u32 {
    10
}

fn default_min_connections() -> u32 {
    1
}

/// Environment deployment tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EnvironmentTag {
    #[default]
    Development,
    Testing,
    Staging,
    Production,
}

impl EnvironmentTag {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Development => "DEV",
            Self::Testing => "TEST",
            Self::Staging => "STAGE",
            Self::Production => "PROD",
        }
    }

    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }
}

/// SSL / TLS connection encryption mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SslMode {
    #[default]
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SslMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Disable => "Disable",
            Self::Prefer => "Prefer",
            Self::Require => "Require",
            Self::VerifyCa => "Verify-CA",
            Self::VerifyFull => "Verify-Full",
        }
    }
}

/// SSH Tunnel configuration for bastion host jumping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SshTunnelConfig {
    pub enabled: bool,
    #[serde(default = "default_ssh_host")]
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub auth_type: SshAuthType,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
}

fn default_ssh_host() -> String {
    "127.0.0.1".to_string()
}

fn default_ssh_port() -> u16 {
    22
}

/// SSH authentication method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SshAuthType {
    #[default]
    Password,
    PrivateKey,
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
            is_read_only: false,
            environment: EnvironmentTag::Development,
            ssl_mode: SslMode::Disable,
            ssh_tunnel: None,
            max_connections: 5,
            min_connections: 1,
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
            is_read_only: false,
            environment: EnvironmentTag::Development,
            ssl_mode: SslMode::Prefer,
            ssh_tunnel: None,
            max_connections: 10,
            min_connections: 1,
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
            is_read_only: false,
            environment: EnvironmentTag::Development,
            ssl_mode: SslMode::Prefer,
            ssh_tunnel: None,
            max_connections: 10,
            min_connections: 1,
        }
    }
}

/// Sort direction for tabular grid columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn toggle(&self) -> Option<Self> {
        match self {
            Self::Ascending => Some(Self::Descending),
            Self::Descending => None,
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
            Self::Float(f) => format!("{:.4}", f)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
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
    pub fn rows(
        columns: Vec<String>,
        column_types: Vec<String>,
        rows: Vec<Vec<QueryValue>>,
    ) -> Self {
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

impl TableInfo {
    pub fn is_view(&self) -> bool {
        self.table_type.to_ascii_uppercase().contains("VIEW")
    }

    /// Dialect-quoted identifier, optionally schema-qualified.
    pub fn qualified_name(&self, family: DatabaseFamily) -> String {
        match self.schema.as_deref() {
            Some(schema) if !schema.is_empty() => {
                format!(
                    "{}.{}",
                    quote_ident(schema, family),
                    quote_ident(&self.name, family)
                )
            }
            _ => quote_ident(&self.name, family),
        }
    }
}

/// Quote an identifier for the given SQL dialect.
pub fn quote_ident(name: &str, family: DatabaseFamily) -> String {
    match family {
        DatabaseFamily::MySql => format!("`{}`", name.replace('`', "``")),
        DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
            format!("\"{}\"", name.replace('"', "\"\""))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_type_properties_and_family() {
        // Embedded
        assert_eq!(DatabaseType::Sqlite.family(), DatabaseFamily::Sqlite);
        assert!(DatabaseType::Sqlite.is_file_based());
        assert_eq!(DatabaseType::Sqlite.category(), DatabaseCategory::Embedded);

        // MySQL Family
        let mysql_types = [
            DatabaseType::Mysql,
            DatabaseType::MariaDB,
            DatabaseType::TiDB,
            DatabaseType::OceanBase,
            DatabaseType::StarRocks,
            DatabaseType::Doris,
            DatabaseType::PolarDB,
            DatabaseType::TDSQL,
            DatabaseType::SelectDB,
            DatabaseType::Databend,
            DatabaseType::GoldenDB,
            DatabaseType::SingleStore,
            DatabaseType::ManticoreSearch,
            DatabaseType::CloudSQLMySQL,
        ];
        for d in mysql_types {
            assert_eq!(d.family(), DatabaseFamily::MySql);
            assert!(!d.is_file_based());
            assert_eq!(d.default_user(), "root");
        }
        assert_eq!(DatabaseType::TiDB.default_port(), 4000);
        assert_eq!(DatabaseType::OceanBase.default_port(), 2881);
        assert_eq!(DatabaseType::StarRocks.default_port(), 9030);
        assert_eq!(DatabaseType::Databend.default_port(), 3307);

        // PostgreSQL Family
        let pg_types = [
            DatabaseType::Postgres,
            DatabaseType::CockroachDB,
            DatabaseType::TimescaleDB,
            DatabaseType::Redshift,
            DatabaseType::YugabyteDB,
            DatabaseType::OpenGauss,
            DatabaseType::Kingbase,
            DatabaseType::GaussDB,
            DatabaseType::Greenplum,
            DatabaseType::QuestDB,
            DatabaseType::Vastbase,
            DatabaseType::YashanDB,
            DatabaseType::HighGo,
            DatabaseType::UXDB,
            DatabaseType::GBase8c,
            DatabaseType::EnterpriseDB,
            DatabaseType::CrateDB,
            DatabaseType::Materialize,
            DatabaseType::AlloyDB,
            DatabaseType::CloudSQLPG,
            DatabaseType::FujitsuPG,
        ];
        for d in pg_types {
            assert_eq!(d.family(), DatabaseFamily::Postgres);
            assert!(!d.is_file_based());
        }
        assert_eq!(DatabaseType::CockroachDB.default_port(), 26257);
        assert_eq!(DatabaseType::Redshift.default_port(), 5439);
        assert_eq!(DatabaseType::OpenGauss.default_port(), 5432);
        assert_eq!(DatabaseType::QuestDB.default_port(), 8812);

        let table = TableInfo {
            name: "ecrm_yb".into(),
            schema: Some("public".into()),
            table_type: "TABLE".into(),
            comment: None,
            row_count_estimate: None,
        };
        assert_eq!(
            table.qualified_name(DatabaseFamily::Postgres),
            "\"public\".\"ecrm_yb\""
        );
        assert_eq!(
            table.qualified_name(DatabaseFamily::MySql),
            "`public`.`ecrm_yb`"
        );

        // All supported count
        assert_eq!(DatabaseType::all().len(), 36);
    }

    #[test]
    fn test_connection_config_serialization_and_defaults() {
        let mut cfg = ConnectionConfig::mysql(
            "Prod Cluster",
            "10.0.0.1",
            3306,
            "billing",
            "app",
            Some("secret".to_string()),
        );
        cfg.environment = EnvironmentTag::Production;
        cfg.is_read_only = true;
        cfg.ssl_mode = SslMode::Require;

        let json = serde_json::to_string(&cfg).expect("serialization should succeed");
        assert!(json.contains("\"production\""));
        assert!(json.contains("\"require\""));
        assert!(json.contains("\"is_read_only\":true"));

        let deserialized: ConnectionConfig =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(deserialized.name, "Prod Cluster");
        assert_eq!(deserialized.environment, EnvironmentTag::Production);
        assert!(deserialized.environment.is_production());
        assert!(deserialized.is_read_only);
        assert_eq!(deserialized.ssl_mode, SslMode::Require);
        assert_eq!(deserialized.max_connections, 10);
    }
}
