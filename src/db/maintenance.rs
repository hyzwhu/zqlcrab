//! Database Storage Capacity Analysis and Table Maintenance Diagnostics Engine.
//!
//! Provides multi-engine (SQLite, MySQL, PostgreSQL) storage inspection
//! (data size, index size, free/bloat space, row counts) and automated
//! table maintenance operations (VACUUM / OPTIMIZE, CHECK, ANALYZE, REINDEX, CHECKSUM).

use crate::db::types::{DatabaseFamily, QueryResult, QueryValue, quote_ident};
use serde::{Deserialize, Serialize};

/// Formats raw byte size into a human-readable display string (B, KB, MB, GB, TB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes < TB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    }
}

/// Storage allocation and health metrics for an individual database table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableStorageMetrics {
    /// Name of the table.
    pub table_name: String,
    /// Schema or database namespace.
    pub schema: Option<String>,
    /// Underlying storage engine (e.g. "InnoDB", "PostgreSQL Heap", "SQLite B-Tree").
    pub engine: Option<String>,
    /// Total physical disk size occupied in bytes (data + indexes + free space).
    pub total_size_bytes: u64,
    /// Data space size occupied by row records in bytes.
    pub data_size_bytes: u64,
    /// Index space size occupied by indexes and B-Trees in bytes.
    pub index_size_bytes: u64,
    /// Free/bloat/fragmented space that can be reclaimed in bytes.
    pub free_size_bytes: u64,
    /// Total number of rows / records (exact or optimizer estimate).
    pub row_count: u64,
    /// Average size per row in bytes.
    pub avg_row_size_bytes: u64,
    /// Dead tuples / zombie rows pending vacuum (PostgreSQL specific).
    pub dead_tuples: Option<u64>,
    /// Collation or encoding for the table.
    pub collation: Option<String>,
    /// Timestamp of last VACUUM operation.
    pub last_vacuum: Option<String>,
    /// Timestamp of last ANALYZE operation.
    pub last_analyze: Option<String>,
}

impl TableStorageMetrics {
    /// Ratio of data size to total size [0.0 .. 1.0].
    pub fn data_ratio(&self) -> f32 {
        if self.total_size_bytes == 0 {
            0.0
        } else {
            (self.data_size_bytes as f32 / self.total_size_bytes as f32).clamp(0.0, 1.0)
        }
    }

    /// Ratio of index size to total size [0.0 .. 1.0].
    pub fn index_ratio(&self) -> f32 {
        if self.total_size_bytes == 0 {
            0.0
        } else {
            (self.index_size_bytes as f32 / self.total_size_bytes as f32).clamp(0.0, 1.0)
        }
    }

    /// Ratio of free/bloat space to total size [0.0 .. 1.0].
    pub fn free_ratio(&self) -> f32 {
        if self.total_size_bytes == 0 {
            0.0
        } else {
            (self.free_size_bytes as f32 / self.total_size_bytes as f32).clamp(0.0, 1.0)
        }
    }
}

/// Aggregated database-wide storage and capacity summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DatabaseStorageMetricsSummary {
    /// Total physical disk footprint across all tables.
    pub total_size_bytes: u64,
    /// Total raw data space across all tables.
    pub total_data_bytes: u64,
    /// Total index space across all tables.
    pub total_index_bytes: u64,
    /// Total free/bloat reclaimable space across all tables.
    pub total_free_bytes: u64,
    /// Total count of inspected tables.
    pub total_tables: usize,
    /// Total aggregated row count.
    pub total_rows: u64,
    /// Detailed list of inspected tables.
    pub tables: Vec<TableStorageMetrics>,
}

impl DatabaseStorageMetricsSummary {
    /// Builds a summary by aggregating a list of individual table storage metrics.
    pub fn from_tables(tables: Vec<TableStorageMetrics>) -> Self {
        let mut total_size_bytes = 0u64;
        let mut total_data_bytes = 0u64;
        let mut total_index_bytes = 0u64;
        let mut total_free_bytes = 0u64;
        let mut total_rows = 0u64;

        for t in &tables {
            total_size_bytes = total_size_bytes.saturating_add(t.total_size_bytes);
            total_data_bytes = total_data_bytes.saturating_add(t.data_size_bytes);
            total_index_bytes = total_index_bytes.saturating_add(t.index_size_bytes);
            total_free_bytes = total_free_bytes.saturating_add(t.free_size_bytes);
            total_rows = total_rows.saturating_add(t.row_count);
        }

        Self {
            total_size_bytes,
            total_data_bytes,
            total_index_bytes,
            total_free_bytes,
            total_tables: tables.len(),
            total_rows,
            tables,
        }
    }
}

/// Automated maintenance and diagnostic actions supported across database engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MaintenanceAction {
    /// Reclaims unused disk space and defragments pages (OPTIMIZE / VACUUM).
    #[default]
    Optimize,
    /// Structural and B-tree consistency check (CHECK TABLE / PRAGMA integrity_check).
    CheckIntegrity,
    /// Refreshes optimizer query distribution statistics (ANALYZE).
    Analyze,
    /// Rebuilds fragmented or corrupted indexes (REINDEX / REPAIR).
    RebuildIndex,
    /// Computes cryptographic or CRC row data checksums (CHECKSUM TABLE).
    Checksum,
}

impl MaintenanceAction {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Optimize => "optimize",
            Self::CheckIntegrity => "check_integrity",
            Self::Analyze => "analyze",
            Self::RebuildIndex => "rebuild_index",
            Self::Checksum => "checksum",
        }
    }

    /// Whether this maintenance action is supported for the given database family.
    pub fn supported_for(&self, family: DatabaseFamily) -> bool {
        match self {
            Self::Optimize => true,
            Self::CheckIntegrity => true,
            Self::Analyze => true,
            Self::RebuildIndex => true,
            Self::Checksum => matches!(
                family,
                DatabaseFamily::MySql | DatabaseFamily::Postgres | DatabaseFamily::Sqlite
            ),
        }
    }

    /// SQL command badge hint according to database dialect.
    pub fn command_hint(&self, family: DatabaseFamily) -> &'static str {
        match (self, family) {
            (Self::Optimize, DatabaseFamily::MySql) => "OPTIMIZE TABLE",
            (Self::Optimize, DatabaseFamily::Postgres) => "VACUUM",
            (Self::Optimize, DatabaseFamily::Sqlite) => "VACUUM",

            (Self::CheckIntegrity, DatabaseFamily::MySql) => "CHECK TABLE",
            (Self::CheckIntegrity, DatabaseFamily::Postgres) => "ANALYZE VERBOSE",
            (Self::CheckIntegrity, DatabaseFamily::Sqlite) => "PRAGMA integrity_check",

            (Self::Analyze, DatabaseFamily::MySql) => "ANALYZE TABLE",
            (Self::Analyze, DatabaseFamily::Postgres) => "ANALYZE",
            (Self::Analyze, DatabaseFamily::Sqlite) => "PRAGMA optimize",

            (Self::RebuildIndex, DatabaseFamily::MySql) => "ALTER TABLE (REPAIR)",
            (Self::RebuildIndex, DatabaseFamily::Postgres) => "REINDEX TABLE",
            (Self::RebuildIndex, DatabaseFamily::Sqlite) => "REINDEX",

            (Self::Checksum, DatabaseFamily::MySql) => "CHECKSUM TABLE",
            (Self::Checksum, DatabaseFamily::Postgres) => "MD5 CHECKSUM",
            (Self::Checksum, DatabaseFamily::Sqlite) => "QUICK CHECK",
        }
    }
}

/// Options configuring table maintenance batch execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceOptions {
    /// Continue running remaining tables even if a single table maintenance step fails.
    pub continue_on_error: bool,
    /// For PostgreSQL: use VACUUM FULL for aggressive space reclamation with table lock.
    pub full_vacuum: bool,
    /// For MySQL: use CHECK TABLE EXTENDED for full consistency checks.
    pub extended_check: bool,
}

impl Default for MaintenanceOptions {
    fn default() -> Self {
        Self {
            continue_on_error: true,
            full_vacuum: false,
            extended_check: true,
        }
    }
}

/// Result status of an individual table maintenance operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaintenanceStatus {
    Success,
    Warning,
    Error,
}

impl MaintenanceStatus {
    pub fn badge_text(&self) -> &'static str {
        match self {
            Self::Success => "[OK]",
            Self::Warning => "[WARN]",
            Self::Error => "[ERR]",
        }
    }
}

/// Audit log entry recorded during table maintenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceLogEntry {
    pub timestamp: String,
    pub table_name: String,
    pub action: MaintenanceAction,
    pub status: MaintenanceStatus,
    pub message: String,
    pub duration_ms: u64,
}

/// Final summary report of a maintenance execution batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSummary {
    pub total_tables: usize,
    pub succeeded: usize,
    pub warnings: usize,
    pub failed: usize,
    pub total_duration_ms: u64,
    pub is_success: bool,
    pub logs: Vec<MaintenanceLogEntry>,
}

/// Real-time progress tracker during batch execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceProgress {
    pub current_index: usize,
    pub total_tables: usize,
    pub current_table: String,
    pub action: MaintenanceAction,
    pub percentage: f32,
}

// -----------------------------------------------------------------------------
// Helper conversion functions for QueryValue
// -----------------------------------------------------------------------------

fn qv_to_u64(val: &QueryValue) -> u64 {
    match val {
        QueryValue::Int(i) => (*i).max(0) as u64,
        QueryValue::Float(f) => (*f).max(0.0) as u64,
        QueryValue::String(s) => s.parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

fn qv_to_opt_string(val: &QueryValue) -> Option<String> {
    match val {
        QueryValue::String(s) if !s.is_empty() => Some(s.clone()),
        QueryValue::DateTime(d) if !d.is_empty() => Some(d.clone()),
        QueryValue::Int(i) => Some(i.to_string()),
        _ => None,
    }
}

// -----------------------------------------------------------------------------
// Storage SQL Query Generator & Result Parser
// -----------------------------------------------------------------------------

/// Generates SQL to inspect physical table storage metrics for the active database.
pub fn generate_storage_query_sql(
    family: DatabaseFamily,
    database_name: Option<&str>,
    schema: Option<&str>,
) -> String {
    match family {
        DatabaseFamily::MySql => {
            let schema_cond = if let Some(db) = database_name.filter(|s| !s.is_empty()) {
                let escaped = db.replace('\'', "''");
                format!("TABLE_SCHEMA = '{escaped}'")
            } else {
                "TABLE_SCHEMA = DATABASE()".to_string()
            };
            format!(
                "SELECT \
                    TABLE_NAME, \
                    ENGINE, \
                    COALESCE(DATA_LENGTH + INDEX_LENGTH + DATA_FREE, 0) AS total_size, \
                    COALESCE(DATA_LENGTH, 0) AS data_size, \
                    COALESCE(INDEX_LENGTH, 0) AS index_size, \
                    COALESCE(DATA_FREE, 0) AS free_size, \
                    COALESCE(TABLE_ROWS, 0) AS row_count, \
                    COALESCE(AVG_ROW_LENGTH, 0) AS avg_row_size, \
                    TABLE_COLLATION, \
                    UPDATE_TIME, \
                    CHECK_TIME \
                FROM information_schema.TABLES \
                WHERE {schema_cond} \
                  AND TABLE_TYPE = 'BASE TABLE' \
                ORDER BY (DATA_LENGTH + INDEX_LENGTH) DESC;"
            )
        }
        DatabaseFamily::Postgres => {
            let schema_cond = if let Some(s) = schema.filter(|s| !s.is_empty()) {
                let escaped = s.replace('\'', "''");
                format!("n.nspname = '{escaped}'")
            } else {
                "n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast') AND n.nspname NOT LIKE 'pg_temp_%'".to_string()
            };
            format!(
                "SELECT \
                    c.relname AS table_name, \
                    n.nspname AS schema_name, \
                    pg_total_relation_size(c.oid) AS total_size, \
                    pg_relation_size(c.oid) AS data_size, \
                    pg_indexes_size(c.oid) AS index_size, \
                    GREATEST(0, (pg_total_relation_size(c.oid) - pg_relation_size(c.oid) - pg_indexes_size(c.oid))) AS free_size, \
                    GREATEST(0, c.reltuples::bigint) AS row_count, \
                    CASE WHEN c.reltuples > 0 THEN (pg_relation_size(c.oid) / c.reltuples)::bigint ELSE 0 END AS avg_row_size, \
                    COALESCE(s.n_dead_tup, 0) AS dead_tuples, \
                    s.last_vacuum::text, \
                    s.last_analyze::text \
                FROM pg_class c \
                JOIN pg_namespace n ON n.oid = c.relnamespace \
                LEFT JOIN pg_stat_user_tables s ON s.relid = c.oid \
                WHERE {schema_cond} \
                  AND c.relkind IN ('r', 'p') \
                ORDER BY pg_total_relation_size(c.oid) DESC;"
            )
        }
        DatabaseFamily::Sqlite => {
            // SQLite storage metrics inspect page_count, page_size, freelist_count via pragma table-valued functions
            "SELECT 'db_overview', 'SQLite', \
                (page_count * page_size) AS total_size, \
                ((page_count - freelist_count) * page_size) AS data_size, \
                0 AS index_size, \
                (freelist_count * page_size) AS free_size, \
                0 AS row_count, \
                0 AS avg_row_size, \
                NULL AS collation, \
                NULL AS last_vacuum, \
                NULL AS last_analyze \
                FROM pragma_page_count(), pragma_page_size(), pragma_freelist_count();"
                .to_string()
        }
    }
}

/// Parses the tabular result set returned by `generate_storage_query_sql`.
pub fn parse_storage_query_result(
    family: DatabaseFamily,
    result: &QueryResult,
) -> Vec<TableStorageMetrics> {
    let mut metrics = Vec::new();

    match family {
        DatabaseFamily::MySql => {
            for row in &result.rows {
                if row.is_empty() {
                    continue;
                }
                let table_name = row
                    .first()
                    .map(|v| v.to_display_string())
                    .unwrap_or_default();
                if table_name.is_empty() {
                    continue;
                }
                let engine = row.get(1).and_then(qv_to_opt_string);
                let total_size_bytes = row.get(2).map(qv_to_u64).unwrap_or(0);
                let data_size_bytes = row.get(3).map(qv_to_u64).unwrap_or(0);
                let index_size_bytes = row.get(4).map(qv_to_u64).unwrap_or(0);
                let free_size_bytes = row.get(5).map(qv_to_u64).unwrap_or(0);
                let row_count = row.get(6).map(qv_to_u64).unwrap_or(0);
                let avg_row_size_bytes = row.get(7).map(qv_to_u64).unwrap_or(0);
                let collation = row.get(8).and_then(qv_to_opt_string);
                let last_analyze = row.get(9).and_then(qv_to_opt_string);
                let last_vacuum = row.get(10).and_then(qv_to_opt_string);

                metrics.push(TableStorageMetrics {
                    table_name,
                    schema: None,
                    engine,
                    total_size_bytes,
                    data_size_bytes,
                    index_size_bytes,
                    free_size_bytes,
                    row_count,
                    avg_row_size_bytes,
                    dead_tuples: None,
                    collation,
                    last_vacuum,
                    last_analyze,
                });
            }
        }
        DatabaseFamily::Postgres => {
            for row in &result.rows {
                if row.is_empty() {
                    continue;
                }
                let table_name = row
                    .first()
                    .map(|v| v.to_display_string())
                    .unwrap_or_default();
                if table_name.is_empty() {
                    continue;
                }
                let schema = row.get(1).and_then(qv_to_opt_string);
                let total_size_bytes = row.get(2).map(qv_to_u64).unwrap_or(0);
                let data_size_bytes = row.get(3).map(qv_to_u64).unwrap_or(0);
                let index_size_bytes = row.get(4).map(qv_to_u64).unwrap_or(0);
                let free_size_bytes = row.get(5).map(qv_to_u64).unwrap_or(0);
                let row_count = row.get(6).map(qv_to_u64).unwrap_or(0);
                let avg_row_size_bytes = row.get(7).map(qv_to_u64).unwrap_or(0);
                let dead_tuples = row.get(8).map(qv_to_u64);
                let last_vacuum = row.get(9).and_then(qv_to_opt_string);
                let last_analyze = row.get(10).and_then(qv_to_opt_string);

                metrics.push(TableStorageMetrics {
                    table_name,
                    schema,
                    engine: Some("PostgreSQL Heap".to_string()),
                    total_size_bytes,
                    data_size_bytes,
                    index_size_bytes,
                    free_size_bytes,
                    row_count,
                    avg_row_size_bytes,
                    dead_tuples,
                    collation: None,
                    last_vacuum,
                    last_analyze,
                });
            }
        }
        DatabaseFamily::Sqlite => {
            for row in &result.rows {
                if row.is_empty() {
                    continue;
                }
                let table_name = row
                    .first()
                    .map(|v| v.to_display_string())
                    .unwrap_or_default();
                let engine = row.get(1).and_then(qv_to_opt_string);
                let total_size_bytes = row.get(2).map(qv_to_u64).unwrap_or(0);
                let data_size_bytes = row.get(3).map(qv_to_u64).unwrap_or(0);
                let index_size_bytes = row.get(4).map(qv_to_u64).unwrap_or(0);
                let free_size_bytes = row.get(5).map(qv_to_u64).unwrap_or(0);
                let row_count = row.get(6).map(qv_to_u64).unwrap_or(0);
                let avg_row_size_bytes = row.get(7).map(qv_to_u64).unwrap_or(0);

                metrics.push(TableStorageMetrics {
                    table_name,
                    schema: None,
                    engine,
                    total_size_bytes,
                    data_size_bytes,
                    index_size_bytes,
                    free_size_bytes,
                    row_count,
                    avg_row_size_bytes,
                    dead_tuples: None,
                    collation: None,
                    last_vacuum: None,
                    last_analyze: None,
                });
            }
        }
    }

    metrics
}

// -----------------------------------------------------------------------------
// Maintenance SQL Statement Generators
// -----------------------------------------------------------------------------

/// Generates maintenance SQL statements for a given table and action.
pub fn generate_maintenance_sql(
    action: MaintenanceAction,
    table: &str,
    family: DatabaseFamily,
    options: &MaintenanceOptions,
    schema: Option<&str>,
) -> Vec<String> {
    let quoted_table = if let Some(s) = schema.filter(|s| !s.is_empty()) {
        format!("{}.{}", quote_ident(s, family), quote_ident(table, family))
    } else {
        quote_ident(table, family)
    };

    match (action, family) {
        (MaintenanceAction::Optimize, DatabaseFamily::MySql) => {
            vec![format!("OPTIMIZE TABLE {quoted_table};")]
        }
        (MaintenanceAction::Optimize, DatabaseFamily::Postgres) => {
            if options.full_vacuum {
                vec![format!("VACUUM FULL {quoted_table};")]
            } else {
                vec![format!("VACUUM (ANALYZE) {quoted_table};")]
            }
        }
        (MaintenanceAction::Optimize, DatabaseFamily::Sqlite) => {
            vec!["VACUUM;".to_string()]
        }

        (MaintenanceAction::CheckIntegrity, DatabaseFamily::MySql) => {
            if options.extended_check {
                vec![format!("CHECK TABLE {quoted_table} EXTENDED;")]
            } else {
                vec![format!("CHECK TABLE {quoted_table};")]
            }
        }
        (MaintenanceAction::CheckIntegrity, DatabaseFamily::Postgres) => {
            vec![format!("ANALYZE VERBOSE {quoted_table};")]
        }
        (MaintenanceAction::CheckIntegrity, DatabaseFamily::Sqlite) => {
            vec![format!("PRAGMA integrity_check({quoted_table});")]
        }

        (MaintenanceAction::Analyze, DatabaseFamily::MySql) => {
            vec![format!("ANALYZE TABLE {quoted_table};")]
        }
        (MaintenanceAction::Analyze, DatabaseFamily::Postgres) => {
            vec![format!("ANALYZE {quoted_table};")]
        }
        (MaintenanceAction::Analyze, DatabaseFamily::Sqlite) => {
            vec![format!("ANALYZE {quoted_table};")]
        }

        (MaintenanceAction::RebuildIndex, DatabaseFamily::MySql) => {
            vec![format!("OPTIMIZE TABLE {quoted_table};")]
        }
        (MaintenanceAction::RebuildIndex, DatabaseFamily::Postgres) => {
            vec![format!("REINDEX TABLE {quoted_table};")]
        }
        (MaintenanceAction::RebuildIndex, DatabaseFamily::Sqlite) => {
            vec![format!("REINDEX {quoted_table};")]
        }

        (MaintenanceAction::Checksum, DatabaseFamily::MySql) => {
            vec![format!("CHECKSUM TABLE {quoted_table} EXTENDED;")]
        }
        (MaintenanceAction::Checksum, DatabaseFamily::Postgres) => {
            vec![format!("SELECT count(*) FROM {quoted_table};")]
        }
        (MaintenanceAction::Checksum, DatabaseFamily::Sqlite) => {
            vec![format!("PRAGMA quick_check({quoted_table});")]
        }
    }
}

/// Parses the execution result of a maintenance query and returns an audit log entry.
pub fn parse_maintenance_result(
    action: MaintenanceAction,
    table_name: &str,
    result: &QueryResult,
    duration_ms: u64,
) -> MaintenanceLogEntry {
    let now = chrono::Local::now().format("%H:%M:%S").to_string();

    let mut message = String::new();
    let mut status = MaintenanceStatus::Success;

    if !result.rows.is_empty() {
        for row in &result.rows {
            let row_str = row
                .iter()
                .map(|v| v.to_display_string())
                .collect::<Vec<_>>()
                .join(" | ");

            let lower = row_str.to_lowercase();
            if lower.contains("error") || lower.contains("corrupt") || lower.contains("fail") {
                status = MaintenanceStatus::Error;
            } else if lower.contains("warn") || lower.contains("note") {
                if status != MaintenanceStatus::Error {
                    status = MaintenanceStatus::Warning;
                }
            }

            if message.is_empty() {
                message = row_str;
            } else {
                message.push_str("; ");
                message.push_str(&row_str);
            }
        }
    }

    if message.is_empty() {
        if let Some(affected) = result.rows_affected {
            message = format!("Completed successfully. Rows affected: {affected}");
        } else {
            message = "Operation completed successfully.".to_string();
        }
    }

    MaintenanceLogEntry {
        timestamp: now,
        table_name: table_name.to_string(),
        action,
        status,
        message,
        duration_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1048576 * 50), "50.00 MB");
        assert_eq!(format_bytes(1073741824 * 2), "2.00 GB");
        assert_eq!(format_bytes(1099511627776 * 3), "3.00 TB");
    }

    #[test]
    fn test_table_storage_metrics_ratios() {
        let t = TableStorageMetrics {
            table_name: "users".to_string(),
            schema: None,
            engine: Some("InnoDB".to_string()),
            total_size_bytes: 1000,
            data_size_bytes: 600,
            index_size_bytes: 300,
            free_size_bytes: 100,
            row_count: 5000,
            avg_row_size_bytes: 120,
            dead_tuples: None,
            collation: Some("utf8mb4_unicode_ci".to_string()),
            last_vacuum: None,
            last_analyze: None,
        };

        assert!((t.data_ratio() - 0.6).abs() < 0.001);
        assert!((t.index_ratio() - 0.3).abs() < 0.001);
        assert!((t.free_ratio() - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_storage_summary_aggregation() {
        let t1 = TableStorageMetrics {
            table_name: "t1".to_string(),
            schema: None,
            engine: None,
            total_size_bytes: 200,
            data_size_bytes: 150,
            index_size_bytes: 50,
            free_size_bytes: 0,
            row_count: 10,
            avg_row_size_bytes: 15,
            dead_tuples: None,
            collation: None,
            last_vacuum: None,
            last_analyze: None,
        };
        let t2 = TableStorageMetrics {
            table_name: "t2".to_string(),
            schema: None,
            engine: None,
            total_size_bytes: 300,
            data_size_bytes: 200,
            index_size_bytes: 50,
            free_size_bytes: 50,
            row_count: 20,
            avg_row_size_bytes: 10,
            dead_tuples: None,
            collation: None,
            last_vacuum: None,
            last_analyze: None,
        };

        let summary = DatabaseStorageMetricsSummary::from_tables(vec![t1, t2]);
        assert_eq!(summary.total_tables, 2);
        assert_eq!(summary.total_size_bytes, 500);
        assert_eq!(summary.total_data_bytes, 350);
        assert_eq!(summary.total_index_bytes, 100);
        assert_eq!(summary.total_free_bytes, 50);
        assert_eq!(summary.total_rows, 30);
    }

    #[test]
    fn test_generate_maintenance_sql() {
        let opts = MaintenanceOptions::default();

        let mysql_opt = generate_maintenance_sql(
            MaintenanceAction::Optimize,
            "users",
            DatabaseFamily::MySql,
            &opts,
            None,
        );
        assert_eq!(mysql_opt, vec!["OPTIMIZE TABLE `users`;"]);

        let pg_opt = generate_maintenance_sql(
            MaintenanceAction::Optimize,
            "users",
            DatabaseFamily::Postgres,
            &opts,
            Some("public"),
        );
        assert_eq!(pg_opt, vec!["VACUUM (ANALYZE) \"public\".\"users\";"]);

        let pg_opt_full = generate_maintenance_sql(
            MaintenanceAction::Optimize,
            "users",
            DatabaseFamily::Postgres,
            &MaintenanceOptions {
                full_vacuum: true,
                ..opts
            },
            None,
        );
        assert_eq!(pg_opt_full, vec!["VACUUM FULL \"users\";"]);

        let sqlite_opt = generate_maintenance_sql(
            MaintenanceAction::Optimize,
            "users",
            DatabaseFamily::Sqlite,
            &opts,
            None,
        );
        assert_eq!(sqlite_opt, vec!["VACUUM;"]);

        let mysql_chk = generate_maintenance_sql(
            MaintenanceAction::CheckIntegrity,
            "orders",
            DatabaseFamily::MySql,
            &opts,
            None,
        );
        assert_eq!(mysql_chk, vec!["CHECK TABLE `orders` EXTENDED;"]);

        let sqlite_chk = generate_maintenance_sql(
            MaintenanceAction::CheckIntegrity,
            "orders",
            DatabaseFamily::Sqlite,
            &opts,
            None,
        );
        assert_eq!(sqlite_chk, vec!["PRAGMA integrity_check(\"orders\");"]);
    }

    #[test]
    fn test_parse_maintenance_result_status() {
        let ok_res = QueryResult {
            columns: vec![
                "Table".to_string(),
                "Op".to_string(),
                "Msg_type".to_string(),
                "Msg_text".to_string(),
            ],
            column_types: vec!["string".to_string(); 4],
            rows: vec![vec![
                QueryValue::String("mydb.users".to_string()),
                QueryValue::String("optimize".to_string()),
                QueryValue::String("status".to_string()),
                QueryValue::String("OK".to_string()),
            ]],
            rows_affected: None,
            execution_time_ms: Some(15),
        };

        let log = parse_maintenance_result(MaintenanceAction::Optimize, "users", &ok_res, 15);
        assert_eq!(log.status, MaintenanceStatus::Success);
        assert!(log.message.contains("OK"));

        let err_res = QueryResult {
            columns: vec!["status".to_string()],
            column_types: vec!["string".to_string()],
            rows: vec![vec![QueryValue::String(
                "*** ERROR: corrupted page 42 ***".to_string(),
            )]],
            rows_affected: None,
            execution_time_ms: Some(5),
        };

        let err_log =
            parse_maintenance_result(MaintenanceAction::CheckIntegrity, "users", &err_res, 5);
        assert_eq!(err_log.status, MaintenanceStatus::Error);
    }

    #[test]
    fn test_sqlite_storage_query() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let sql = generate_storage_query_sql(DatabaseFamily::Sqlite, None, None);
        let stmt = conn.prepare(&sql);
        assert!(
            stmt.is_ok(),
            "SQLite storage query prepare failed: {:?}",
            stmt.err()
        );
        let mut stmt = stmt.unwrap();
        let mut rows = stmt.query([]).unwrap();
        assert!(rows.next().unwrap().is_some());
    }
}
