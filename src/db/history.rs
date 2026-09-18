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

        let content = fs::read_to_string(&self.storage_path).map_err(|e| DbError::Io(e.to_string()))?;
        let items: Vec<QueryHistoryItem> = serde_json::from_str(&content).map_err(|e| DbError::Configuration(e.to_string()))?;
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
        let temp_dir = std::env::temp_dir().join(format!("zqlcrab_hist_test_{}", uuid::Uuid::new_v4()));
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
}
