//! Persistent query history tracking and search.

use crate::db::error::{DbError, DbResult};
use crate::db::types::DatabaseType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Query execution outcome status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryHistoryStatus {
    Success,
    Error,
}

/// Query history status filter criteria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryStatusFilter {
    #[default]
    All,
    SuccessOnly,
    ErrorOnly,
    SlowOnly,
    StarredOnly,
}

impl HistoryStatusFilter {
    pub const ALL: [HistoryStatusFilter; 5] = [
        Self::All,
        Self::SuccessOnly,
        Self::ErrorOnly,
        Self::SlowOnly,
        Self::StarredOnly,
    ];
}

/// Export format for query history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryExportFormat {
    Csv,
    Json,
    Sql,
}

/// A logged database query with performance and diagnostic metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryItem {
    pub id: String,
    pub query_text: String,
    pub timestamp: DateTime<Utc>,
    pub execution_duration_ms: u64,
    pub status: QueryHistoryStatus,
    pub rows_affected: Option<u64>,
    pub rows_returned: Option<usize>,
    pub error_message: Option<String>,
    pub connection_id: Option<String>,
    pub connection_name: Option<String>,
    pub database_type: Option<DatabaseType>,
    #[serde(default)]
    pub is_favorite: bool,
}

impl QueryHistoryItem {
    pub fn success(
        query_text: String,
        duration_ms: u64,
        rows_returned: Option<usize>,
        rows_affected: Option<u64>,
        conn_id: Option<String>,
        conn_name: Option<String>,
        db_type: Option<DatabaseType>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            query_text,
            timestamp: Utc::now(),
            execution_duration_ms: duration_ms,
            status: QueryHistoryStatus::Success,
            rows_affected,
            rows_returned,
            error_message: None,
            connection_id: conn_id,
            connection_name: conn_name,
            database_type: db_type,
            is_favorite: false,
        }
    }

    pub fn error(
        query_text: String,
        duration_ms: u64,
        error_msg: String,
        conn_id: Option<String>,
        conn_name: Option<String>,
        db_type: Option<DatabaseType>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            query_text,
            timestamp: Utc::now(),
            execution_duration_ms: duration_ms,
            status: QueryHistoryStatus::Error,
            rows_affected: None,
            rows_returned: None,
            error_message: Some(error_msg),
            connection_id: conn_id,
            connection_name: conn_name,
            database_type: db_type,
            is_favorite: false,
        }
    }
}

/// In-memory and local disk manager for query history logs.
pub struct QueryHistoryManager {
    items: Vec<QueryHistoryItem>,
    storage_path: PathBuf,
    max_entries: usize,
}

impl QueryHistoryManager {
    /// Initialized with default OS config directory `~/.config/zqlcrab/query_history.json`.
    pub fn new() -> Self {
        let storage_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zqlcrab")
            .join("query_history.json");

        let mut mgr = Self {
            items: Vec::new(),
            storage_path,
            max_entries: 500,
        };
        let _ = mgr.load();
        mgr
    }

    /// Custom storage path constructor for isolated testing.
    pub fn with_storage_path(path: PathBuf, max_entries: usize) -> Self {
        let mut mgr = Self {
            items: Vec::new(),
            storage_path: path,
            max_entries,
        };
        let _ = mgr.load();
        mgr
    }

    /// Load history items from disk.
    pub fn load(&mut self) -> DbResult<()> {
        if !self.storage_path.exists() {
            return Ok(());
        }

        let content =
            fs::read_to_string(&self.storage_path).map_err(|e| DbError::Io(e.to_string()))?;
        let items: Vec<QueryHistoryItem> =
            serde_json::from_str(&content).map_err(|e| DbError::Configuration(e.to_string()))?;
        self.items = items;
        if self.items.len() > self.max_entries {
            self.items.truncate(self.max_entries);
        }
        Ok(())
    }

    /// Save current history items to disk.
    pub fn save(&self) -> DbResult<()> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).map_err(|e| DbError::Io(e.to_string()))?;
        }

        let json = serde_json::to_string_pretty(&self.items)
            .map_err(|e| DbError::Configuration(e.to_string()))?;
        fs::write(&self.storage_path, json).map_err(|e| DbError::Io(e.to_string()))?;
        Ok(())
    }

    /// Append a new item at the top (newest first) and maintain the FIFO capacity.
    pub fn record(&mut self, item: QueryHistoryItem) {
        self.items.insert(0, item);
        if self.items.len() > self.max_entries {
            self.items.truncate(self.max_entries);
        }
        let _ = self.save();
    }

    /// Update maximum capacity and truncate excess entries if needed.
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max;
        if self.items.len() > self.max_entries {
            self.items.truncate(self.max_entries);
            let _ = self.save();
        }
    }

    /// Get all history items.
    pub fn list(&self) -> &[QueryHistoryItem] {
        &self.items
    }

    /// Alias for list() to retrieve slice of items.
    pub fn items(&self) -> &[QueryHistoryItem] {
        &self.items
    }

    /// Search queries containing the keyword (case-insensitive) in SQL text or connection name.
    pub fn search(&self, keyword: &str) -> Vec<&QueryHistoryItem> {
        if keyword.is_empty() {
            return self.items.iter().collect();
        }

        let kw = keyword.to_lowercase();
        self.items
            .iter()
            .filter(|it| {
                it.query_text.to_lowercase().contains(&kw)
                    || it
                        .connection_name
                        .as_deref()
                        .map(|cn| cn.to_lowercase().contains(&kw))
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Clear all history records.
    pub fn clear(&mut self) -> DbResult<()> {
        self.items.clear();
        self.save()
    }

    /// Toggle favorite (starred) status for a specific query history item.
    pub fn toggle_favorite(&mut self, id: &str) -> DbResult<bool> {
        let mut new_state = false;
        let mut found = false;
        for item in &mut self.items {
            if item.id == id {
                item.is_favorite = !item.is_favorite;
                new_state = item.is_favorite;
                found = true;
                break;
            }
        }
        if found {
            self.save()?;
        }
        Ok(new_state)
    }

    /// Delete a single query history item by id.
    pub fn delete_item(&mut self, id: &str) -> DbResult<bool> {
        let prev_len = self.items.len();
        self.items.retain(|it| it.id != id);
        let removed = self.items.len() < prev_len;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Export history records as formatted JSON.
    pub fn export_as_json(&self, items: &[&QueryHistoryItem]) -> String {
        serde_json::to_string_pretty(items).unwrap_or_default()
    }

    /// Export history records as standard CSV format.
    pub fn export_as_csv(&self, items: &[&QueryHistoryItem]) -> String {
        let mut wtr = csv::Writer::from_writer(vec![]);
        let _ = wtr.write_record([
            "ID",
            "Timestamp",
            "Status",
            "DurationMs",
            "RowsAffected",
            "RowsReturned",
            "DatabaseType",
            "ConnectionName",
            "Favorite",
            "QueryText",
            "ErrorMessage",
        ]);

        for item in items {
            let status = match item.status {
                QueryHistoryStatus::Success => "success",
                QueryHistoryStatus::Error => "error",
            };
            let rows_affected = item
                .rows_affected
                .map(|n| n.to_string())
                .unwrap_or_default();
            let rows_returned = item
                .rows_returned
                .map(|n| n.to_string())
                .unwrap_or_default();
            let db_type = item
                .database_type
                .map(|d| d.display_name())
                .unwrap_or_default();
            let conn_name = item.connection_name.as_deref().unwrap_or_default();
            let favorite = if item.is_favorite { "true" } else { "false" };
            let err_msg = item.error_message.as_deref().unwrap_or_default();

            let _ = wtr.write_record([
                &item.id,
                &item.timestamp.to_rfc3339(),
                status,
                &item.execution_duration_ms.to_string(),
                &rows_affected,
                &rows_returned,
                db_type,
                conn_name,
                favorite,
                &item.query_text,
                err_msg,
            ]);
        }

        wtr.into_inner()
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .unwrap_or_default()
    }

    /// Export history records as an executable or archivable SQL script.
    pub fn export_as_sql(&self, items: &[&QueryHistoryItem]) -> String {
        let mut out = String::new();
        out.push_str("-- ========================================================\n");
        out.push_str("-- zqlcrab Query Execution Audit History Dump\n");
        out.push_str(&format!("-- Exported At: {}\n", Utc::now().to_rfc3339()));
        out.push_str(&format!("-- Total Statements: {}\n", items.len()));
        out.push_str("-- ========================================================\n\n");

        for (idx, item) in items.iter().enumerate() {
            let status_str = match item.status {
                QueryHistoryStatus::Success => "SUCCESS",
                QueryHistoryStatus::Error => "ERROR",
            };
            out.push_str(&format!(
                "-- [{}] Timestamp: {} | Status: {} | Duration: {}ms",
                idx + 1,
                item.timestamp.to_rfc3339(),
                status_str,
                item.execution_duration_ms
            ));
            if let Some(conn) = &item.connection_name {
                out.push_str(&format!(" | Connection: {}", conn));
            }
            if let Some(db) = &item.database_type {
                out.push_str(&format!(" | Dialect: {}", db.display_name()));
            }
            if let Some(aff) = item.rows_affected {
                out.push_str(&format!(" | Affected: {}", aff));
            }
            if let Some(ret) = item.rows_returned {
                out.push_str(&format!(" | Rows: {}", ret));
            }
            if item.is_favorite {
                out.push_str(" | [STARRED]");
            }
            out.push('\n');

            if let Some(err) = &item.error_message {
                out.push_str(&format!("-- Error: {}\n", err.replace('\n', " ")));
            }

            let trimmed_sql = item.query_text.trim();
            out.push_str(trimmed_sql);
            if !trimmed_sql.ends_with(';') {
                out.push(';');
            }
            out.push_str("\n\n");
        }

        out
    }
}

impl Default for QueryHistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_manager_fifo_and_search() {
        let temp_dir =
            std::env::temp_dir().join(format!("zqlcrab_hist_test_{}", uuid::Uuid::new_v4()));
        let storage_file = temp_dir.join("history.json");

        let mut mgr = QueryHistoryManager::with_storage_path(storage_file.clone(), 3);
        assert_eq!(mgr.list().len(), 0);

        // Record 4 queries (capacity 3)
        mgr.record(QueryHistoryItem::success(
            "SELECT 1".to_string(),
            5,
            Some(1),
            None,
            None,
            Some("Local SQLite".to_string()),
            Some(DatabaseType::Sqlite),
        ));
        mgr.record(QueryHistoryItem::success(
            "SELECT * FROM users".to_string(),
            12,
            Some(10),
            None,
            None,
            Some("Production Postgres".to_string()),
            Some(DatabaseType::Postgres),
        ));
        mgr.record(QueryHistoryItem::error(
            "DROP DATABASE invalid".to_string(),
            2,
            "Access denied".to_string(),
            None,
            None,
            None,
        ));
        mgr.record(QueryHistoryItem::success(
            "SELECT count(*) FROM orders".to_string(),
            20,
            Some(1),
            None,
            None,
            Some("Production Postgres".to_string()),
            Some(DatabaseType::Postgres),
        ));

        // Capacity capped at 3, "SELECT 1" should be truncated
        assert_eq!(mgr.list().len(), 3);
        assert_eq!(mgr.list()[0].query_text, "SELECT count(*) FROM orders");
        assert_eq!(mgr.list()[1].query_text, "DROP DATABASE invalid");
        assert_eq!(mgr.list()[2].query_text, "SELECT * FROM users");

        // Keyword search
        let orders_match = mgr.search("orders");
        assert_eq!(orders_match.len(), 1);
        assert_eq!(orders_match[0].query_text, "SELECT count(*) FROM orders");

        let pg_match = mgr.search("Production");
        assert_eq!(pg_match.len(), 2);

        // Reload from file to verify persistence
        let reloaded = QueryHistoryManager::with_storage_path(storage_file.clone(), 3);
        assert_eq!(reloaded.list().len(), 3);

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_history_item_favorites_deletion_and_export() {
        let temp_dir =
            std::env::temp_dir().join(format!("zqlcrab_hist_test_{}", uuid::Uuid::new_v4()));
        let storage_file = temp_dir.join("history_ext.json");

        let mut mgr = QueryHistoryManager::with_storage_path(storage_file.clone(), 10);

        let item1 = QueryHistoryItem::success(
            "SELECT * FROM customers WHERE active = 1".to_string(),
            42,
            Some(100),
            None,
            Some("conn-1".to_string()),
            Some("Prod PG".to_string()),
            Some(DatabaseType::Postgres),
        );
        let id1 = item1.id.clone();
        mgr.record(item1);

        let item2 = QueryHistoryItem::error(
            "UPDATE accounts SET balance = 0".to_string(),
            550,
            "permission denied on accounts table".to_string(),
            Some("conn-2".to_string()),
            Some("Staging MySQL".to_string()),
            Some(DatabaseType::Mysql),
        );
        let id2 = item2.id.clone();
        mgr.record(item2);

        assert_eq!(mgr.list().len(), 2);
        assert!(!mgr.list()[0].is_favorite);

        // Toggle favorite for item2
        let fav = mgr.toggle_favorite(&id2).expect("toggle should succeed");
        assert!(fav);
        assert!(mgr.list()[0].is_favorite);

        // Verify JSON export
        let all_refs: Vec<&QueryHistoryItem> = mgr.list().iter().collect();
        let json_export = mgr.export_as_json(&all_refs);
        assert!(json_export.contains("customers"));
        assert!(json_export.contains("accounts"));
        assert!(json_export.contains("\"is_favorite\": true"));

        // Verify CSV export
        let csv_export = mgr.export_as_csv(&all_refs);
        assert!(csv_export.contains("ID,Timestamp,Status"));
        assert!(csv_export.contains("PostgreSQL"));
        assert!(csv_export.contains("Staging MySQL"));
        assert!(csv_export.contains("permission denied"));

        // Verify SQL export
        let sql_export = mgr.export_as_sql(&all_refs);
        assert!(sql_export.contains("zqlcrab Query Execution Audit History Dump"));
        assert!(sql_export.contains("[STARRED]"));
        assert!(sql_export.contains("SELECT * FROM customers WHERE active = 1;"));
        assert!(sql_export.contains("UPDATE accounts SET balance = 0;"));

        // Delete single item1
        let removed = mgr.delete_item(&id1).expect("delete should succeed");
        assert!(removed);
        assert_eq!(mgr.list().len(), 1);
        assert_eq!(mgr.list()[0].id, id2);

        // Reload to test persistence of deletion and favorite
        let reloaded = QueryHistoryManager::with_storage_path(storage_file.clone(), 10);
        assert_eq!(reloaded.list().len(), 1);
        assert!(reloaded.list()[0].is_favorite);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
