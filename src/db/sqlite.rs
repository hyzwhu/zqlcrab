//! SQLite database adapter implementation using rusqlite.

use crate::db::{
    adapter::DatabaseAdapter,
    error::{DbError, DbResult},
    types::{
        ColumnInfo, ConnectionConfig, ConnectionStatus, DatabaseSchema, IndexInfo, QueryResult,
        QueryValue, TableInfo,
    },
};
use async_trait::async_trait;
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Adapter for SQLite databases (file-based or in-memory).
pub struct SqliteAdapter {
    config: ConnectionConfig,
    conn: Option<Arc<Mutex<Connection>>>,
}

impl SqliteAdapter {
    pub fn new(config: ConnectionConfig) -> Self {
        Self { config, conn: None }
    }

    /// Converts a rusqlite ValueRef to our uniform QueryValue.
    fn convert_value(val: ValueRef<'_>) -> QueryValue {
        match val {
            ValueRef::Null => QueryValue::Null,
            ValueRef::Integer(i) => QueryValue::Int(i),
            ValueRef::Real(f) => QueryValue::Float(f),
            ValueRef::Text(t) => QueryValue::String(String::from_utf8_lossy(t).to_string()),
            ValueRef::Blob(b) => QueryValue::Bytes(b.to_vec()),
        }
    }
}

#[async_trait]
impl DatabaseAdapter for SqliteAdapter {
    async fn connect(&mut self) -> DbResult<()> {
        let db_path = self.config.database.trim();
        let conn = if db_path.is_empty() || db_path == ":memory:" {
            Connection::open_in_memory()
                .map_err(|e| DbError::connection(format!("Failed to open in-memory SQLite: {e}")))?
        } else {
            // Ensure parent directory exists
            if let Some(parent) = Path::new(db_path).parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            Connection::open_with_flags(
                db_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|e| DbError::connection(format!("Failed to open SQLite database at '{db_path}': {e}")))?
        };

        // Enable foreign keys and WAL journal mode
        let _ = conn.execute("PRAGMA foreign_keys = ON;", []);
        if db_path != ":memory:" && !db_path.is_empty() {
            let _ = conn.query_row("PRAGMA journal_mode = WAL;", [], |_| Ok(()));
        }

        self.conn = Some(Arc::new(Mutex::new(conn)));
        Ok(())
    }

    async fn disconnect(&mut self) -> DbResult<()> {
        self.conn = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.conn.is_some()
    }

    async fn test_connection(&self) -> DbResult<ConnectionStatus> {
        let start = Instant::now();
        let conn_arc = self.conn.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let conn = conn_arc.lock().map_err(|e| DbError::PoolError(e.to_string()))?;

        let version: String = conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .map_err(|e| DbError::query(format!("Failed to query SQLite version: {e}")))?;

        let ping_ms = start.elapsed().as_millis() as u64;

        Ok(ConnectionStatus {
            connected: true,
            server_version: Some(format!("SQLite {version}")),
            current_database: Some(self.config.database.clone()),
            ping_ms: Some(ping_ms),
        })
    }

    async fn execute_query(&self, sql: &str) -> DbResult<QueryResult> {
        let start = Instant::now();
        let conn_arc = self.conn.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let conn = conn_arc.lock().map_err(|e| DbError::PoolError(e.to_string()))?;

        let trimmed = sql.trim();
        let is_select = trimmed.len() >= 6
            && (trimmed[..6].eq_ignore_ascii_case("SELECT")
                || trimmed[..6].eq_ignore_ascii_case("PRAGMA")
                || trimmed[..6].eq_ignore_ascii_case("EXPLAIN")
                || (trimmed.len() >= 4 && trimmed[..4].eq_ignore_ascii_case("WITH")));

        if is_select {
            let mut stmt = conn
                .prepare(sql)
                .map_err(|e| DbError::query(format!("Failed to prepare query: {e}")))?;

            let columns: Vec<String> = stmt
                .column_names()
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            let col_count = columns.len();
            let column_types: Vec<String> = vec!["TEXT".to_string(); col_count];

            let mut rows_iter = stmt
                .query([])
                .map_err(|e| DbError::query(format!("Failed to execute query: {e}")))?;

            let mut rows = Vec::new();
            while let Some(row) = rows_iter.next().map_err(|e| DbError::query(e.to_string()))? {
                let mut row_vals = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let val = row.get_ref(i).map_err(|e| DbError::query(e.to_string()))?;
                    row_vals.push(Self::convert_value(val));
                }
                rows.push(row_vals);
            }

            let execution_time_ms = start.elapsed().as_millis() as u64;
            Ok(QueryResult {
                columns,
                column_types,
                rows,
                rows_affected: None,
                execution_time_ms: Some(execution_time_ms),
            })
        } else {
            let affected = conn
                .execute(sql, [])
                .map_err(|e| DbError::query(format!("Statement execution error: {e}")))? as u64;
            let execution_time_ms = start.elapsed().as_millis() as u64;

            Ok(QueryResult {
                columns: Vec::new(),
                column_types: Vec::new(),
                rows: Vec::new(),
                rows_affected: Some(affected),
                execution_time_ms: Some(execution_time_ms),
            })
        }
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseSchema>> {
        Ok(vec![DatabaseSchema {
            name: if self.config.database.is_empty() {
                "main".to_string()
            } else {
                self.config.database.clone()
            },
            is_system: false,
        }])
    }

    async fn list_tables(
        &self,
        _database: Option<&str>,
        _schema: Option<&str>,
    ) -> DbResult<Vec<TableInfo>> {
        let conn_arc = self.conn.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let conn = conn_arc.lock().map_err(|e| DbError::PoolError(e.to_string()))?;

        let sql = "SELECT name, type FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name;";
        let mut stmt = conn.prepare(sql).map_err(|e| DbError::query(e.to_string()))?;

        let mut tables = Vec::new();
        let mut rows = stmt.query([]).map_err(|e| DbError::query(e.to_string()))?;
        while let Some(row) = rows.next().map_err(|e| DbError::query(e.to_string()))? {
            let name: String = row.get(0).map_err(|e| DbError::query(e.to_string()))?;
            let raw_type: String = row.get(1).map_err(|e| DbError::query(e.to_string()))?;
            tables.push(TableInfo {
                name,
                schema: Some("main".to_string()),
                table_type: raw_type.to_uppercase(),
                comment: None,
                row_count_estimate: None,
            });
        }
        Ok(tables)
    }

    async fn list_columns(
        &self,
        _database: Option<&str>,
        _schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnInfo>> {
        let conn_arc = self.conn.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let conn = conn_arc.lock().map_err(|e| DbError::PoolError(e.to_string()))?;

        let clean_table = table.replace('"', "\"\"");
        let sql = format!("PRAGMA table_info(\"{clean_table}\");");
        let mut stmt = conn.prepare(&sql).map_err(|e| DbError::query(e.to_string()))?;

        let mut columns = Vec::new();
        let mut rows = stmt.query([]).map_err(|e| DbError::query(e.to_string()))?;
        while let Some(row) = rows.next().map_err(|e| DbError::query(e.to_string()))? {
            let name: String = row.get(1).map_err(|e| DbError::query(e.to_string()))?;
            let data_type: String = row.get(2).map_err(|e| DbError::query(e.to_string()))?;
            let not_null: i32 = row.get(3).map_err(|e| DbError::query(e.to_string()))?;
            let default_val: Option<String> = row.get(4).unwrap_or(None);
            let pk: i32 = row.get(5).map_err(|e| DbError::query(e.to_string()))?;

            columns.push(ColumnInfo {
                name,
                data_type: if data_type.is_empty() {
                    "ANY".to_string()
                } else {
                    data_type
                },
                is_nullable: not_null == 0,
                is_primary_key: pk > 0,
                is_auto_increment: false,
                default_value: default_val,
                description: None,
            });
        }
        Ok(columns)
    }

    async fn list_indexes(
        &self,
        _database: Option<&str>,
        _schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<IndexInfo>> {
        let conn_arc = self.conn.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let conn = conn_arc.lock().map_err(|e| DbError::PoolError(e.to_string()))?;

        let clean_table = table.replace('"', "\"\"");
        let sql = format!("PRAGMA index_list(\"{clean_table}\");");
        let mut stmt = conn.prepare(&sql).map_err(|e| DbError::query(e.to_string()))?;

        let mut indexes = Vec::new();
        let mut rows = stmt.query([]).map_err(|e| DbError::query(e.to_string()))?;
        while let Some(row) = rows.next().map_err(|e| DbError::query(e.to_string()))? {
            let name: String = row.get(1).map_err(|e| DbError::query(e.to_string()))?;
            let unique: i32 = row.get(2).map_err(|e| DbError::query(e.to_string()))?;
            let origin: String = row.get(3).unwrap_or_default();

            indexes.push(IndexInfo {
                name,
                table_name: table.to_string(),
                columns: Vec::new(),
                is_unique: unique == 1,
                is_primary: origin == "pk",
            });
        }
        Ok(indexes)
    }

    async fn get_table_ddl(
        &self,
        _database: Option<&str>,
        _schema: Option<&str>,
        table: &str,
    ) -> DbResult<Option<String>> {
        let conn_arc = self.conn.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let conn = conn_arc.lock().map_err(|e| DbError::PoolError(e.to_string()))?;

        let sql = "SELECT sql FROM sqlite_master WHERE name = ?1;";
        let ddl: Option<String> = conn.query_row(sql, [table], |row| row.get(0)).unwrap_or(None);
        Ok(ddl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_adapter_full_lifecycle() {
        let config = ConnectionConfig::sqlite("test_mem", ":memory:");
        let mut adapter = SqliteAdapter::new(config);

        // Connect
        adapter.connect().await.expect("connect should succeed");
        assert!(adapter.is_connected());

        // Test connection
        let status = adapter.test_connection().await.expect("test_connection should succeed");
        assert!(status.connected);
        assert!(status.server_version.unwrap().starts_with("SQLite"));

        // Create table
        let ddl = "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, age INTEGER, active BOOLEAN);";
        let create_res = adapter.execute_query(ddl).await.expect("create table should succeed");
        assert_eq!(create_res.rows_affected, Some(0));

        // Insert rows
        let insert_sql = "INSERT INTO users (name, age, active) VALUES ('Alice', 30, 1), ('Bob', 25, 0);";
        let insert_res = adapter.execute_query(insert_sql).await.expect("insert should succeed");
        assert_eq!(insert_res.rows_affected, Some(2));

        // Query rows
        let select_sql = "SELECT id, name, age, active FROM users ORDER BY id ASC;";
        let select_res = adapter.execute_query(select_sql).await.expect("select should succeed");
        assert_eq!(select_res.columns, vec!["id", "name", "age", "active"]);
        assert_eq!(select_res.rows.len(), 2);
        assert_eq!(select_res.rows[0][1], QueryValue::String("Alice".to_string()));
        assert_eq!(select_res.rows[0][2], QueryValue::Int(30));
        assert_eq!(select_res.rows[1][1], QueryValue::String("Bob".to_string()));

        // List tables
        let tables = adapter.list_tables(None, None).await.expect("list_tables should succeed");
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");

        // List columns
        let cols = adapter.list_columns(None, None, "users").await.expect("list_columns should succeed");
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[0].name, "id");
        assert!(cols[0].is_primary_key);
        assert_eq!(cols[1].name, "name");
        assert!(!cols[1].is_nullable);

        // Disconnect
        adapter.disconnect().await.expect("disconnect should succeed");
        assert!(!adapter.is_connected());
    }
}

