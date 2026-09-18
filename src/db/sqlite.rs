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

    fn should_seed_demo(config: &ConnectionConfig) -> bool {
        let path = config.database.trim();
        (path.is_empty() || path == ":memory:") && config.name.contains("Sample")
    }

    fn seed_demo_schema(conn: &Connection) -> DbResult<()> {
        let already_seeded: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('users', 'products', 'reports')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if already_seeded > 0 {
            return Ok(());
        }

        conn.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT,
                active INTEGER NOT NULL DEFAULT 1
            );
            INSERT INTO users (name, email, active) VALUES
                ('Alice', 'alice@example.com', 1),
                ('Bob', 'bob@example.com', 1),
                ('Carol', 'carol@example.com', 0);

            CREATE TABLE products (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                price REAL NOT NULL,
                stock INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO products (name, price, stock) VALUES
                ('Keyboard', 79.0, 12),
                ('Mouse', 29.5, 40),
                ('Monitor', 249.0, 8);

            CREATE TABLE reports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                report_url TEXT NOT NULL,
                create_time TEXT NOT NULL,
                rsrcr_names TEXT NOT NULL,
                title TEXT NOT NULL
            );
            INSERT INTO reports (report_url, create_time, rsrcr_names, title) VALUES
                ('http://ecrm2.gf.com.cn/app/v2/api/att/downloadFtpReport?file_path=/ecrm/ecrm-att/2026/09/17/202609170008GH0024.pdf&client_name=ecrm2&file_name=202609170008GH0024.pdf', '2026-09-17 14:00:00', '【周绍南】202609170008GH0024', '【华泰资产配置】"924"两周年:来时路与再出发'),
                ('http://ecrm2.gf.com.cn/app/v2/api/att/downloadFtpReport?file_path=/ecrm/ecrm-att/2026/09/16/202609160002GH0032.pdf&client_name=ecrm2&file_name=202609160002GH0032.pdf', '2026-09-16 10:30:00', '【刘伟】202609160002GH0032', '【宏观专题】全球制造业景气度跟踪与资产配置展望'),
                ('http://ecrm2.gf.com.cn/app/v2/api/att/downloadFtpReport?file_path=/ecrm/ecrm-att/2026/09/15/202609150002GH0011.pdf&client_name=ecrm2&file_name=202609150002GH0011.pdf', '2026-09-15 09:15:00', '【张继强】202609150002GH0011', '【债券周报】流动性跟踪周报');
            "#,
        )
        .map_err(|e| DbError::query(format!("Failed to seed sample SQLite schema: {e}")))?;
        Ok(())
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
            let flags = if self.config.is_read_only {
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
            } else {
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
            };
            Connection::open_with_flags(db_path, flags)
                .map_err(|e| DbError::connection(format!("Failed to open SQLite database at '{db_path}': {e}")))?
        };

        let _ = conn.busy_timeout(std::time::Duration::from_secs(self.config.connect_timeout_secs.max(1)));

        // Enable foreign keys and WAL journal mode (if writable)
        if !self.config.is_read_only {
            let _ = conn.execute("PRAGMA foreign_keys = ON;", []);
            if db_path != ":memory:" && !db_path.is_empty() {
                let _ = conn.query_row("PRAGMA journal_mode = WAL;", [], |_| Ok(()));
            }
        }

        if Self::should_seed_demo(&self.config) {
            Self::seed_demo_schema(&conn)?;
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

        let is_select = crate::db::safety::QuerySafetyValidator::is_result_set_query(sql);

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

    async fn execute_batch(&self, sql: &str) -> DbResult<()> {
        let conn_arc = self.conn.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let conn = conn_arc.lock().map_err(|e| DbError::PoolError(e.to_string()))?;
        conn.execute_batch(sql)
            .map_err(|e| DbError::query(format!("Batch execution failed: {e}")))?;
        Ok(())
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

    #[tokio::test]
    async fn test_sample_memory_db_seeds_demo_tables() {
        let config = ConnectionConfig::sqlite("Sample SQLite (In-Memory)", ":memory:");
        let mut adapter = SqliteAdapter::new(config);
        adapter.connect().await.expect("connect should succeed");

        let tables = adapter.list_tables(None, None).await.expect("list_tables");
        let names: Vec<_> = tables.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"users"), "expected users table, got {names:?}");
        assert!(names.contains(&"products"), "expected products table, got {names:?}");

        let users = adapter
            .execute_query("SELECT COUNT(*) AS n FROM users")
            .await
            .expect("count users");
        assert_eq!(users.rows[0][0], QueryValue::Int(3));
    }

    #[tokio::test]
    async fn test_comment_prefixed_select_returns_rows() {
        let config = ConnectionConfig::sqlite("test_comments", ":memory:");
        let mut adapter = SqliteAdapter::new(config);
        adapter.connect().await.expect("connect should succeed");

        let sql = "-- CrabStudio SQL Workspace\n-- Type your SQL queries here and press ⌘↵ or Run\nSELECT 1 AS id, 'Welcome to CrabStudio' AS message;";
        let result = adapter.execute_query(sql).await.expect("comment-prefixed SELECT should run");
        assert_eq!(result.columns, vec!["id", "message"]);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], QueryValue::Int(1));
        assert_eq!(
            result.rows[0][1],
            QueryValue::String("Welcome to CrabStudio".to_string())
        );
    }

    #[tokio::test]
    async fn test_explain_query_plan_parses_scan() {
        let config = ConnectionConfig::sqlite("test_explain", ":memory:");
        let mut adapter = SqliteAdapter::new(config);
        adapter.connect().await.expect("connect should succeed");
        adapter
            .execute_query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);")
            .await
            .expect("create table");

        let sql = crate::db::explain::wrap_explain_sql(
            "SELECT * FROM users LIMIT 100",
            crate::db::types::DatabaseFamily::Sqlite,
        );
        let result = adapter.execute_query(&sql).await.expect("explain should run");
        let plan = crate::db::explain::parse_explain_result(
            crate::db::types::DatabaseFamily::Sqlite,
            &result,
        );
        assert!(
            plan.node_count() >= 1,
            "expected at least one plan node, raw={}",
            plan.raw
        );
        assert!(
            plan.flatten()
                .iter()
                .any(|(_, n)| n.node_type.contains("Scan") || n.details.to_uppercase().contains("SCAN")),
            "expected a scan node, got {:?}",
            plan.roots
        );
    }

    #[tokio::test]
    async fn test_sqlite_batch_execution_with_changeset() {
        let config = ConnectionConfig::sqlite("test_batch", ":memory:");
        let mut adapter = SqliteAdapter::new(config);
        adapter.connect().await.expect("connect should succeed");

        adapter
            .execute_query("CREATE TABLE items (id INTEGER PRIMARY KEY, title TEXT, price REAL);")
            .await
            .expect("create table");

        adapter
            .execute_query("INSERT INTO items (id, title, price) VALUES (1, 'Book', 19.99), (2, 'Pen', 2.50);")
            .await
            .expect("insert items");

        let initial = adapter.execute_query("SELECT id, title, price FROM items ORDER BY id;").await.expect("query items");
        let cols = adapter.list_columns(None, None, "items").await.expect("list_columns");

        let mut cs = crate::db::changeset::GridChangeset::new();
        // Update row 0 title to 'Hardcover Book'
        cs.stage_cell_update(0, 1, "title", initial.rows[0][1].clone(), QueryValue::String("Hardcover Book".into()));
        // Delete row 1 (Pen)
        cs.toggle_delete_row(1, &initial.rows[1]);

        let plan = crate::db::sql_gen::generate_review_plan(
            "items",
            None,
            crate::db::types::DatabaseFamily::Sqlite,
            &cols,
            &initial.columns,
            &initial.rows,
            &cs,
        );

        assert_eq!(plan.updates_count, 1);
        assert_eq!(plan.deletes_count, 1);
        assert!(plan.has_primary_key);

        // Execute batch transaction
        adapter.execute_batch(&plan.full_script).await.expect("execute_batch should succeed");

        // Verify changes applied
        let updated = adapter.execute_query("SELECT id, title, price FROM items ORDER BY id;").await.expect("query items after batch");
        assert_eq!(updated.rows.len(), 1);
        assert_eq!(updated.rows[0][0], QueryValue::Int(1));
        assert_eq!(updated.rows[0][1], QueryValue::String("Hardcover Book".into()));
    }

    #[tokio::test]
    async fn test_sqlite_batch_execution_with_new_row_insert() {
        let config = ConnectionConfig::sqlite("test_insert_batch", ":memory:");
        let mut adapter = SqliteAdapter::new(config);
        adapter.connect().await.expect("connect should succeed");

        adapter
            .execute_query("CREATE TABLE contacts (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, email TEXT);")
            .await
            .expect("create contacts table");

        adapter
            .execute_query("INSERT INTO contacts (name, email) VALUES ('Alice', 'alice@old.com');")
            .await
            .expect("insert initial contact");

        let initial = adapter
            .execute_query("SELECT id, name, email FROM contacts ORDER BY id;")
            .await
            .expect("query initial contacts");
        let cols = adapter.list_columns(None, None, "contacts").await.expect("list columns");

        let mut cs = crate::db::changeset::GridChangeset::new();
        // 1. Stage an update to Alice's email
        cs.set_cell_value(
            0,
            2,
            "email".to_string(),
            QueryValue::String("alice@old.com".into()),
            QueryValue::String("alice@newcorp.com".into()),
        );

        // 2. Stage a newly inserted row for Bob (leaving id as Null for auto-increment)
        cs.add_inserted_row(vec![
            QueryValue::Null,
            QueryValue::String("Bob".into()),
            QueryValue::String("bob@example.com".into()),
        ]);

        let (updates, deletes, inserts) = cs.change_summary();
        assert_eq!(updates, 1);
        assert_eq!(deletes, 0);
        assert_eq!(inserts, 1);

        let plan = crate::db::sql_gen::generate_review_plan(
            "contacts",
            None,
            crate::db::types::DatabaseFamily::Sqlite,
            &cols,
            &initial.columns,
            &initial.rows,
            &cs,
        );

        assert_eq!(plan.inserts_count, 1);
        assert_eq!(plan.updates_count, 1);
        assert!(plan.has_primary_key);

        // Verify that the generated INSERT statement omitted the auto-increment id column
        assert!(plan.full_script.contains("INSERT INTO \"contacts\" (\"name\", \"email\") VALUES ('Bob', 'bob@example.com');"));
        // Verify UPDATE is present
        assert!(plan.full_script.contains("UPDATE \"contacts\""));
        assert!(plan.full_script.contains("SET \"email\" = 'alice@newcorp.com'"));

        // Execute batch transaction atomically
        adapter.execute_batch(&plan.full_script).await.expect("execute_batch should succeed");

        // Query contacts to verify both rows are present and accurate
        let reloaded = adapter
            .execute_query("SELECT id, name, email FROM contacts ORDER BY id ASC;")
            .await
            .expect("query contacts after commit");

        assert_eq!(reloaded.rows.len(), 2);
        // Alice
        assert_eq!(reloaded.rows[0][0], QueryValue::Int(1));
        assert_eq!(reloaded.rows[0][1], QueryValue::String("Alice".into()));
        assert_eq!(reloaded.rows[0][2], QueryValue::String("alice@newcorp.com".into()));
        // Bob
        assert_eq!(reloaded.rows[1][0], QueryValue::Int(2));
        assert_eq!(reloaded.rows[1][1], QueryValue::String("Bob".into()));
        assert_eq!(reloaded.rows[1][2], QueryValue::String("bob@example.com".into()));
    }
}

