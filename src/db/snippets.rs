//! SQL Snippets and query templates catalog with multi-dialect diagnostics and persistence.

use crate::db::error::{DbError, DbResult};
use crate::db::types::DatabaseFamily;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Category of SQL snippet or template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SnippetCategory {
    Performance,
    Maintenance,
    Schema,
    Template,
    #[default]
    Custom,
}

impl SnippetCategory {
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Performance => "Performance & Locks",
            Self::Maintenance => "Maintenance & Health",
            Self::Schema => "Schema & Indexes",
            Self::Template => "Query Template",
            Self::Custom => "Custom Snippet",
        }
    }

    pub fn icon_badge(&self) -> &'static str {
        match self {
            Self::Performance => "⚡",
            Self::Maintenance => "🛠️",
            Self::Schema => "📐",
            Self::Template => "📋",
            Self::Custom => "⭐",
        }
    }
}

/// A stored SQL snippet or query template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlSnippet {
    pub id: String,
    pub title: String,
    pub description: String,
    pub sql: String,
    pub dialect: Option<DatabaseFamily>,
    pub category: SnippetCategory,
    pub is_built_in: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SqlSnippet {
    pub fn new_custom(
        title: String,
        description: String,
        sql: String,
        dialect: Option<DatabaseFamily>,
        category: SnippetCategory,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            description,
            sql,
            dialect,
            category,
            is_built_in: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn built_in(
        id: &'static str,
        title: &'static str,
        description: &'static str,
        sql: &'static str,
        dialect: Option<DatabaseFamily>,
        category: SnippetCategory,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            sql: sql.trim().to_string(),
            dialect,
            category,
            is_built_in: true,
            created_at: now,
            updated_at: now,
        }
    }
}

/// In-memory and local disk manager for SQL Snippets.
pub struct SnippetManager {
    built_in: Vec<SqlSnippet>,
    custom_snippets: Vec<SqlSnippet>,
    storage_path: PathBuf,
}

impl SnippetManager {
    /// Initialize with built-in templates and load custom snippets from OS config directory.
    pub fn new() -> Self {
        let storage_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zqlcrab")
            .join("snippets.json");

        let mut mgr = Self {
            built_in: generate_built_in_snippets(),
            custom_snippets: Vec::new(),
            storage_path,
        };
        let _ = mgr.load();
        mgr
    }

    /// Construct with a custom storage path (useful for testing).
    pub fn with_storage_path(storage_path: PathBuf) -> Self {
        let mut mgr = Self {
            built_in: generate_built_in_snippets(),
            custom_snippets: Vec::new(),
            storage_path,
        };
        let _ = mgr.load();
        mgr
    }

    /// Load custom snippets from disk.
    pub fn load(&mut self) -> DbResult<()> {
        if !self.storage_path.exists() {
            return Ok(());
        }

        let content =
            fs::read_to_string(&self.storage_path).map_err(|e| DbError::Io(e.to_string()))?;
        let items: Vec<SqlSnippet> =
            serde_json::from_str(&content).map_err(|e| DbError::Configuration(e.to_string()))?;
        self.custom_snippets = items;
        Ok(())
    }

    /// Persist custom snippets to disk.
    pub fn save(&self) -> DbResult<()> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).map_err(|e| DbError::Io(e.to_string()))?;
        }

        let json = serde_json::to_string_pretty(&self.custom_snippets)
            .map_err(|e| DbError::Configuration(e.to_string()))?;
        fs::write(&self.storage_path, json).map_err(|e| DbError::Io(e.to_string()))?;
        Ok(())
    }

    /// List all snippets (custom first, followed by built-in).
    pub fn list_all(&self) -> Vec<SqlSnippet> {
        let mut all = self.custom_snippets.clone();
        all.extend(self.built_in.clone());
        all
    }

    /// Search and filter snippets by keyword, dialect, and category.
    pub fn search(
        &self,
        keyword: &str,
        dialect_filter: Option<DatabaseFamily>,
        category_filter: Option<SnippetCategory>,
    ) -> Vec<SqlSnippet> {
        let kw = keyword.trim().to_lowercase();
        self.list_all()
            .into_iter()
            .filter(|s| {
                if let Some(cat) = category_filter
                    && s.category != cat
                {
                    return false;
                }
                if let Some(target_dialect) = dialect_filter
                    && let Some(snip_dialect) = s.dialect
                    && snip_dialect != target_dialect
                {
                    return false;
                }
                if !kw.is_empty() {
                    let matches_title = s.title.to_lowercase().contains(&kw);
                    let matches_desc = s.description.to_lowercase().contains(&kw);
                    let matches_sql = s.sql.to_lowercase().contains(&kw);
                    if !matches_title && !matches_desc && !matches_sql {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Add a new custom snippet and persist it.
    pub fn add_custom(
        &mut self,
        title: String,
        description: String,
        sql: String,
        dialect: Option<DatabaseFamily>,
        category: SnippetCategory,
    ) -> SqlSnippet {
        let snippet = SqlSnippet::new_custom(title, description, sql, dialect, category);
        self.custom_snippets.insert(0, snippet.clone());
        let _ = self.save();
        snippet
    }

    /// Update an existing custom snippet.
    pub fn update_custom(&mut self, updated: SqlSnippet) -> DbResult<()> {
        if let Some(item) = self
            .custom_snippets
            .iter_mut()
            .find(|s| s.id == updated.id && !s.is_built_in)
        {
            item.title = updated.title;
            item.description = updated.description;
            item.sql = updated.sql;
            item.dialect = updated.dialect;
            item.category = updated.category;
            item.updated_at = Utc::now();
            self.save()?;
            Ok(())
        } else {
            Err(DbError::NotFound(format!(
                "Custom snippet not found: {}",
                updated.id
            )))
        }
    }

    /// Delete a custom snippet by ID.
    pub fn delete_custom(&mut self, id: &str) -> DbResult<()> {
        let initial_len = self.custom_snippets.len();
        self.custom_snippets.retain(|s| s.id != id);
        if self.custom_snippets.len() < initial_len {
            self.save()?;
            Ok(())
        } else {
            Err(DbError::NotFound(format!(
                "Snippet not found or cannot delete built-in: {}",
                id
            )))
        }
    }

    /// Get custom snippets count.
    pub fn custom_count(&self) -> usize {
        self.custom_snippets.len()
    }

    /// Get built-in snippets count.
    pub fn built_in_count(&self) -> usize {
        self.built_in.len()
    }
}

impl Default for SnippetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate production-grade, dialect-aware diagnostic, maintenance, and template snippets.
fn generate_built_in_snippets() -> Vec<SqlSnippet> {
    vec![
        // ------------------ PostgreSQL Performance & Diagnostics ------------------
        SqlSnippet::built_in(
            "pg-active-queries",
            "Active Queries & Connections",
            "List currently running queries with client IP, connection duration, and state",
            r#"SELECT
    pid,
    usename AS username,
    client_addr,
    now() - query_start AS query_duration,
    state,
    wait_event_type,
    wait_event,
    query
FROM pg_stat_activity
WHERE state != 'idle'
  AND pid != pg_backend_pid()
ORDER BY query_start ASC;"#,
            Some(DatabaseFamily::Postgres),
            SnippetCategory::Performance,
        ),
        SqlSnippet::built_in(
            "pg-blocked-locks",
            "Blocked Locks & Deadlock Chains",
            "Find queries blocked by other processes and identify blocking transaction pids",
            r#"SELECT
    blocked_locks.pid     AS blocked_pid,
    blocked_activity.usename AS blocked_user,
    blocking_locks.pid    AS blocking_pid,
    blocking_activity.usename AS blocking_user,
    blocked_activity.query AS blocked_statement,
    blocking_activity.query AS current_statement_in_blocking_process
FROM pg_catalog.pg_locks blocked_locks
JOIN pg_catalog.pg_stat_activity blocked_activity ON blocked_activity.pid = blocked_locks.pid
JOIN pg_catalog.pg_locks blocking_locks
    ON blocking_locks.locktype = blocked_locks.locktype
    AND blocking_locks.database IS NOT DISTINCT FROM blocked_locks.database
    AND blocking_locks.relation IS NOT DISTINCT FROM blocked_locks.relation
    AND blocking_locks.page IS NOT DISTINCT FROM blocked_locks.page
    AND blocking_locks.tuple IS NOT DISTINCT FROM blocked_locks.tuple
    AND blocking_locks.virtualxid IS NOT DISTINCT FROM blocked_locks.virtualxid
    AND blocking_locks.transactionid IS NOT DISTINCT FROM blocked_locks.transactionid
    AND blocking_locks.classid IS NOT DISTINCT FROM blocked_locks.classid
    AND blocking_locks.objid IS NOT DISTINCT FROM blocked_locks.objid
    AND blocking_locks.objsubid IS NOT DISTINCT FROM blocked_locks.objsubid
    AND blocking_locks.pid != blocked_locks.pid
JOIN pg_catalog.pg_stat_activity blocking_activity ON blocking_activity.pid = blocking_locks.pid
WHERE NOT blocked_locks.granted;"#,
            Some(DatabaseFamily::Postgres),
            SnippetCategory::Performance,
        ),
        SqlSnippet::built_in(
            "pg-table-sizes",
            "Top Tables by Disk Space",
            "Inspect physical table volume, index disk usage, and total size across schemas",
            r#"SELECT
    schemaname,
    relname AS table_name,
    pg_size_pretty(pg_total_relation_size(relid)) AS total_size,
    pg_size_pretty(pg_relation_size(relid)) AS table_size,
    pg_size_pretty(pg_indexes_size(relid)) AS index_size
FROM pg_catalog.pg_statio_user_tables
ORDER BY pg_total_relation_size(relid) DESC
LIMIT 20;"#,
            Some(DatabaseFamily::Postgres),
            SnippetCategory::Maintenance,
        ),
        SqlSnippet::built_in(
            "pg-unused-indexes",
            "Unused & Low-Scan Indexes",
            "Detect candidate indexes consuming space with zero or minimal index scans",
            r#"SELECT
    schemaname,
    relname AS table_name,
    indexrelname AS index_name,
    idx_scan AS scan_count,
    pg_size_pretty(pg_relation_size(indexrelid)) AS index_size
FROM pg_stat_user_indexes
WHERE idx_scan = 0
  AND indexrelname NOT LIKE '%_pkey'
ORDER BY pg_relation_size(indexrelid) DESC
LIMIT 30;"#,
            Some(DatabaseFamily::Postgres),
            SnippetCategory::Maintenance,
        ),
        SqlSnippet::built_in(
            "pg-cache-hit-ratio",
            "Buffer Cache Hit Ratio",
            "Evaluate database memory buffer cache efficiency (target is usually > 99%)",
            r#"SELECT
    'index hit rate' AS name,
    (sum(idx_blks_hit)) / nullif(sum(idx_blks_hit + idx_blks_read), 0) * 100 AS ratio
FROM pg_statio_user_indexes
UNION ALL
SELECT
    'table hit rate' AS name,
    sum(heap_blks_hit) / nullif(sum(heap_blks_hit + heap_blks_read), 0) * 100 AS ratio
FROM pg_statio_user_tables;"#,
            Some(DatabaseFamily::Postgres),
            SnippetCategory::Performance,
        ),
        SqlSnippet::built_in(
            "pg-dead-tuples-vacuum",
            "Table Bloat & Dead Tuples",
            "View live rows vs dead tuples to determine if manual VACUUM ANALYZE is recommended",
            r#"SELECT
    schemaname,
    relname AS table_name,
    n_live_tup AS live_tuples,
    n_dead_tup AS dead_tuples,
    round(n_dead_tup * 100.0 / nullif(n_live_tup + n_dead_tup, 0), 2) AS dead_tuple_ratio_pct,
    last_vacuum,
    last_autovacuum
FROM pg_stat_user_tables
WHERE n_dead_tup > 500
ORDER BY n_dead_tup DESC
LIMIT 25;"#,
            Some(DatabaseFamily::Postgres),
            SnippetCategory::Maintenance,
        ),
        // ------------------ MySQL Performance & Diagnostics ------------------
        SqlSnippet::built_in(
            "mysql-running-threads",
            "Active Processlist & Threads",
            "Monitor currently running database connections, active queries, and durations",
            r#"SELECT
    ID,
    USER,
    HOST,
    DB,
    COMMAND,
    TIME AS duration_seconds,
    STATE,
    LEFT(INFO, 200) AS current_query
FROM information_schema.PROCESSLIST
WHERE COMMAND != 'Sleep'
ORDER BY TIME DESC;"#,
            Some(DatabaseFamily::MySql),
            SnippetCategory::Performance,
        ),
        SqlSnippet::built_in(
            "mysql-table-sizes",
            "Top Tables by Size in Database",
            "Examine data and index storage footprint per table for the active database",
            r#"SELECT
    table_name AS `Table`,
    round(((data_length + index_length) / 1024 / 1024), 2) AS `Total Size (MB)`,
    round(((data_length) / 1024 / 1024), 2) AS `Data Size (MB)`,
    round(((index_length) / 1024 / 1024), 2) AS `Index Size (MB)`,
    table_rows AS `Est. Rows`
FROM information_schema.TABLES
WHERE table_schema = DATABASE()
ORDER BY (data_length + index_length) DESC
LIMIT 25;"#,
            Some(DatabaseFamily::MySql),
            SnippetCategory::Maintenance,
        ),
        SqlSnippet::built_in(
            "mysql-innodb-locks",
            "InnoDB Transactions & Lock Waits",
            "Inspect uncommitted transactions and active lock waits in InnoDB storage engine",
            r#"SELECT
    trx.trx_id,
    trx.trx_state,
    trx.trx_started,
    now() - trx.trx_started AS duration_seconds,
    trx.trx_rows_locked,
    trx.trx_rows_modified,
    trx.trx_query
FROM information_schema.innodb_trx trx
ORDER BY trx.trx_started ASC;"#,
            Some(DatabaseFamily::MySql),
            SnippetCategory::Performance,
        ),
        SqlSnippet::built_in(
            "mysql-thread-stats",
            "Server Threads & Connection Health",
            "Check global thread allocation and connected client statistics",
            r#"SHOW GLOBAL STATUS WHERE Variable_name IN (
    'Threads_connected',
    'Threads_running',
    'Threads_created',
    'Threads_cached',
    'Max_used_connections',
    'Aborted_connects'
);"#,
            Some(DatabaseFamily::MySql),
            SnippetCategory::Maintenance,
        ),
        // ------------------ SQLite Maintenance & Schema ------------------
        SqlSnippet::built_in(
            "sqlite-integrity-check",
            "Database Integrity Check",
            "Verify SQLite database b-tree consistency and check for corrupt pages",
            "PRAGMA integrity_check;",
            Some(DatabaseFamily::Sqlite),
            SnippetCategory::Maintenance,
        ),
        SqlSnippet::built_in(
            "sqlite-table-sizes",
            "Table Page Count & Schema Overview",
            "List all tables, views, and schemas stored in sqlite_master",
            r#"SELECT
    type,
    name AS object_name,
    tbl_name AS parent_table,
    sql
FROM sqlite_master
WHERE type IN ('table', 'view', 'index')
ORDER BY type, name;"#,
            Some(DatabaseFamily::Sqlite),
            SnippetCategory::Schema,
        ),
        SqlSnippet::built_in(
            "sqlite-vacuum-reindex",
            "Vacuum & Database Optimization",
            "Rebuild database file to reclaim unused space and run query planner optimization",
            r#"VACUUM;
PRAGMA optimize;"#,
            Some(DatabaseFamily::Sqlite),
            SnippetCategory::Maintenance,
        ),
        SqlSnippet::built_in(
            "sqlite-fk-check",
            "Foreign Key Violations Check",
            "Scan the entire SQLite database to report any broken foreign key constraints",
            "PRAGMA foreign_key_check;",
            Some(DatabaseFamily::Sqlite),
            SnippetCategory::Maintenance,
        ),
        // ------------------ Generic Query Templates ------------------
        SqlSnippet::built_in(
            "template-paged-query",
            "Keyset / Offset Pagination Template",
            "Standard pagination structure with ordering and row limits",
            r#"SELECT *
FROM "your_table"
WHERE "id" > 0
ORDER BY "id" ASC
LIMIT 50 OFFSET 0;"#,
            None,
            SnippetCategory::Template,
        ),
        SqlSnippet::built_in(
            "template-find-duplicates",
            "Find Duplicate Records",
            "Detect duplicate rows grouped by target key columns with count",
            r#"SELECT
    "target_column",
    COUNT(*) AS "occurrence_count"
FROM "your_table"
GROUP BY "target_column"
HAVING COUNT(*) > 1
ORDER BY "occurrence_count" DESC;"#,
            None,
            SnippetCategory::Template,
        ),
        SqlSnippet::built_in(
            "template-conditional-update",
            "Conditional Batch Update Template",
            "Safely update multiple records matching specific predicates with transaction safeguard",
            r#"UPDATE "your_table"
SET
    "status" = 'processed',
    "updated_at" = CURRENT_TIMESTAMP
WHERE "status" = 'pending'
  AND "created_at" >= '2026-01-01';"#,
            None,
            SnippetCategory::Template,
        ),
        SqlSnippet::built_in(
            "template-group-summary",
            "Summary Aggregation & Group Stats",
            "Calculate summary metrics (Total, Average, Min, Max) across categorical groups",
            r#"SELECT
    "category_column",
    COUNT(*) AS total_count,
    ROUND(AVG("numeric_column"), 2) AS avg_value,
    MIN("numeric_column") AS min_value,
    MAX("numeric_column") AS max_value
FROM "your_table"
GROUP BY "category_column"
ORDER BY total_count DESC;"#,
            None,
            SnippetCategory::Template,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_built_in_snippets_loaded() {
        let snippets = generate_built_in_snippets();
        assert!(!snippets.is_empty());
        assert!(snippets.len() >= 15);

        // Verify dialects present
        let has_pg = snippets
            .iter()
            .any(|s| s.dialect == Some(DatabaseFamily::Postgres));
        let has_mysql = snippets
            .iter()
            .any(|s| s.dialect == Some(DatabaseFamily::MySql));
        let has_sqlite = snippets
            .iter()
            .any(|s| s.dialect == Some(DatabaseFamily::Sqlite));
        let has_generic = snippets.iter().any(|s| s.dialect.is_none());

        assert!(has_pg);
        assert!(has_mysql);
        assert!(has_sqlite);
        assert!(has_generic);
    }

    #[test]
    fn test_snippet_manager_crud() {
        let temp_dir = env::temp_dir().join(format!("zqlcrab_snip_test_{}", Uuid::new_v4()));
        let storage_path = temp_dir.join("snippets.json");

        let mut mgr = SnippetManager::with_storage_path(storage_path.clone());
        assert_eq!(mgr.custom_count(), 0);
        let built_in_count = mgr.built_in_count();
        assert!(built_in_count > 0);

        // Add custom snippet
        let added = mgr.add_custom(
            "My Custom Check".into(),
            "Custom query description".into(),
            "SELECT 1 FROM test;".into(),
            Some(DatabaseFamily::Postgres),
            SnippetCategory::Custom,
        );
        assert_eq!(mgr.custom_count(), 1);

        // Search by keyword
        let search_res = mgr.search("Custom Check", None, None);
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].id, added.id);

        // Update custom snippet
        let mut updated = added.clone();
        updated.title = "Updated Custom Check".into();
        assert!(mgr.update_custom(updated).is_ok());

        let search_res2 = mgr.search("Updated Custom Check", None, None);
        assert_eq!(search_res2.len(), 1);

        // Delete custom snippet
        assert!(mgr.delete_custom(&added.id).is_ok());
        assert_eq!(mgr.custom_count(), 0);

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_search_filters() {
        let mgr = SnippetManager::new();

        // Filter by dialect
        let pg_only = mgr.search("", Some(DatabaseFamily::Postgres), None);
        for s in pg_only {
            assert!(s.dialect == Some(DatabaseFamily::Postgres) || s.dialect.is_none());
        }

        // Filter by category
        let perf_only = mgr.search("", None, Some(SnippetCategory::Performance));
        for s in perf_only {
            assert_eq!(s.category, SnippetCategory::Performance);
        }
    }
}
