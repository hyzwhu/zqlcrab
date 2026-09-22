//! MySQL database adapter implementation using mysql_async.

use crate::db::{
    adapter::DatabaseAdapter,
    error::{DbError, DbResult},
    types::{
        ColumnInfo, ConnectionConfig, ConnectionStatus, DatabaseSchema, IndexInfo, QueryResult,
        QueryValue, TableInfo,
    },
};
use async_trait::async_trait;
use mysql_async::{Opts, OptsBuilder, Pool, Value, consts::ColumnType, prelude::Queryable};
use std::time::Instant;

pub struct MysqlAdapter {
    config: ConnectionConfig,
    pool: Option<Pool>,
}

impl MysqlAdapter {
    pub fn new(config: ConnectionConfig) -> Self {
        Self { config, pool: None }
    }

    /// Converts a mysql_async Value to uniform QueryValue, guided by optional ColumnType.
    fn convert_typed_value(val: Value, col_type: Option<ColumnType>) -> QueryValue {
        match val {
            Value::NULL => QueryValue::Null,
            Value::Bytes(b) => {
                if let Ok(s) = String::from_utf8(b.clone()) {
                    if let Some(ct) = col_type {
                        match ct {
                            ColumnType::MYSQL_TYPE_TINY
                            | ColumnType::MYSQL_TYPE_SHORT
                            | ColumnType::MYSQL_TYPE_LONG
                            | ColumnType::MYSQL_TYPE_LONGLONG
                            | ColumnType::MYSQL_TYPE_INT24
                            | ColumnType::MYSQL_TYPE_YEAR => {
                                if let Ok(i) = s.trim().parse::<i64>() {
                                    return QueryValue::Int(i);
                                }
                            }
                            ColumnType::MYSQL_TYPE_FLOAT
                            | ColumnType::MYSQL_TYPE_DOUBLE
                            | ColumnType::MYSQL_TYPE_DECIMAL
                            | ColumnType::MYSQL_TYPE_NEWDECIMAL => {
                                if let Ok(f) = s.trim().parse::<f64>() {
                                    return QueryValue::Float(f);
                                }
                            }
                            ColumnType::MYSQL_TYPE_DATE
                            | ColumnType::MYSQL_TYPE_DATETIME
                            | ColumnType::MYSQL_TYPE_TIMESTAMP => {
                                return QueryValue::DateTime(s);
                            }
                            _ => {}
                        }
                    }
                    QueryValue::String(s)
                } else {
                    QueryValue::Bytes(b)
                }
            }
            Value::Int(i) => QueryValue::Int(i),
            Value::UInt(u) => QueryValue::Int(u as i64),
            Value::Float(f) => QueryValue::Float(f as f64),
            Value::Double(d) => QueryValue::Float(d),
            Value::Date(y, m, d, h, i, s, _u) => {
                if let Some(ColumnType::MYSQL_TYPE_DATE) = col_type {
                    QueryValue::DateTime(format!("{y:04}-{m:02}-{d:02}"))
                } else if _u > 0 {
                    QueryValue::DateTime(format!(
                        "{y:04}-{m:02}-{d:02} {h:02}:{i:02}:{s:02}.{_u:06}"
                    ))
                } else {
                    QueryValue::DateTime(format!("{y:04}-{m:02}-{d:02} {h:02}:{i:02}:{s:02}"))
                }
            }
            Value::Time(is_neg, d, h, m, s, _u) => {
                let sign = if is_neg { "-" } else { "" };
                let total_hours = (d as u32) * 24 + (h as u32);
                if _u > 0 {
                    QueryValue::String(format!("{sign}{total_hours:02}:{m:02}:{s:02}.{_u:06}"))
                } else {
                    QueryValue::String(format!("{sign}{total_hours:02}:{m:02}:{s:02}"))
                }
            }
        }
    }

    #[allow(dead_code)]
    fn convert_value(val: Value) -> QueryValue {
        Self::convert_typed_value(val, None)
    }
}

#[async_trait]
impl DatabaseAdapter for MysqlAdapter {
    async fn connect(&mut self) -> DbResult<()> {
        let mut builder = OptsBuilder::default();
        if !self.config.host.is_empty() {
            builder = builder.ip_or_hostname(self.config.host.clone());
        }
        if self.config.port > 0 {
            builder = builder.tcp_port(self.config.port);
        }
        if !self.config.username.is_empty() {
            builder = builder.user(Some(self.config.username.clone()));
        }
        if let Some(ref pwd) = self.config.password {
            builder = builder.pass(Some(pwd.clone()));
        }
        if !self.config.database.is_empty() {
            builder = builder.db_name(Some(self.config.database.clone()));
        }

        // Force connection charset to utf8mb4 to prevent latin1/mojibake on multibyte UTF-8 characters (Chinese, emoji)
        builder = builder.init(vec!["SET NAMES utf8mb4;".to_string()]);

        let opts: Opts = builder.into();
        let pool = Pool::new(opts);

        let timeout_dur = std::time::Duration::from_secs(self.config.connect_timeout_secs.max(3));
        let connect_fut = async {
            let mut conn = pool
                .get_conn()
                .await
                .map_err(|e| DbError::connection(format!("Failed to connect to MySQL: {e}")))?;

            conn.ping()
                .await
                .map_err(|e| DbError::connection(format!("Failed to ping MySQL: {e}")))?;

            Ok(())
        };

        match tokio::time::timeout(timeout_dur, connect_fut).await {
            Ok(Ok(())) => {
                self.pool = Some(pool);
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(DbError::connection(format!(
                "Connection to MySQL at {}:{} timed out after {}s",
                self.config.host,
                self.config.port,
                timeout_dur.as_secs()
            ))),
        }
    }

    async fn disconnect(&mut self) -> DbResult<()> {
        if let Some(pool) = self.pool.take() {
            let _ = pool.disconnect().await;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    async fn test_connection(&self) -> DbResult<ConnectionStatus> {
        let start = Instant::now();
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;

        let row: Option<(String, Option<String>)> = conn
            .query_first("SELECT VERSION(), DATABASE();")
            .await
            .map_err(|e| DbError::query(format!("MySQL test query failed: {e}")))?;

        let (version, curr_db) = row.unwrap_or_else(|| ("Unknown".to_string(), None));
        let ping_ms = start.elapsed().as_millis() as u64;

        Ok(ConnectionStatus {
            connected: true,
            server_version: Some(format!("MySQL {version}")),
            current_database: curr_db,
            ping_ms: Some(ping_ms),
        })
    }

    async fn execute_query(&self, sql: &str) -> DbResult<QueryResult> {
        let start = Instant::now();
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;

        let statements = crate::db::sql_gen::split_sql_statements(sql);
        if statements.is_empty() {
            return Ok(QueryResult {
                columns: Vec::new(),
                column_types: Vec::new(),
                rows: Vec::new(),
                rows_affected: Some(0),
                execution_time_ms: Some(start.elapsed().as_millis() as u64),
            });
        }

        let mut last_result: Option<QueryResult> = None;
        let mut total_affected: u64 = 0;
        let total_stmts = statements.len();

        for (idx, stmt_str) in statements.iter().enumerate() {
            let is_select = crate::db::safety::QuerySafetyValidator::is_result_set_query(stmt_str);
            let snippet = if stmt_str.len() > 60 {
                format!("{}...", &stmt_str[..60].replace('\n', " "))
            } else {
                stmt_str.replace('\n', " ")
            };

            if is_select {
                let mut query_result = conn
                    .query_iter(stmt_str.as_str())
                    .await
                    .map_err(|e| {
                        if total_stmts > 1 {
                            DbError::query(format!(
                                "Statement {}/{} query failed [{}]: {e}",
                                idx + 1,
                                total_stmts,
                                snippet
                            ))
                        } else {
                            DbError::query(format!("Query failed: {e}"))
                        }
                    })?;

                let columns: Vec<String> = query_result
                    .columns()
                    .map(|cols| cols.iter().map(|c| c.name_str().to_string()).collect())
                    .unwrap_or_default();
                let col_count = columns.len();
                let column_types: Vec<String> = query_result
                    .columns()
                    .map(|cols| {
                        cols.iter()
                            .map(|c| format!("{:?}", c.column_type()))
                            .collect()
                    })
                    .unwrap_or_default();

                let col_types: Vec<ColumnType> = query_result
                    .columns()
                    .map(|cols| cols.iter().map(|c| c.column_type()).collect())
                    .unwrap_or_default();

                let rows_raw = query_result
                    .collect::<mysql_async::Row>()
                    .await
                    .map_err(|e| {
                        if total_stmts > 1 {
                            DbError::query(format!(
                                "Statement {}/{} collect rows failed [{}]: {e}",
                                idx + 1,
                                total_stmts,
                                snippet
                            ))
                        } else {
                            DbError::query(format!("Failed to collect rows: {e}"))
                        }
                    })?;

                let mut rows = Vec::with_capacity(rows_raw.len());
                for r in rows_raw {
                    let mut row_vals = Vec::with_capacity(col_count);
                    for i in 0..col_count {
                        let val: Value = r.get(i).unwrap_or(Value::NULL);
                        let ct = col_types.get(i).copied();
                        row_vals.push(Self::convert_typed_value(val, ct));
                    }
                    rows.push(row_vals);
                }

                last_result = Some(QueryResult {
                    columns,
                    column_types,
                    rows,
                    rows_affected: None,
                    execution_time_ms: None,
                });
            } else {
                conn.query_drop(stmt_str.as_str())
                    .await
                    .map_err(|e| {
                        if total_stmts > 1 {
                            DbError::query(format!(
                                "Statement {}/{} execution failed [{}]: {e}",
                                idx + 1,
                                total_stmts,
                                snippet
                            ))
                        } else {
                            DbError::query(format!("Statement execution failed: {e}"))
                        }
                    })?;
                total_affected += conn.affected_rows();
            }
        }

        let execution_time_ms = start.elapsed().as_millis() as u64;

        if let Some(mut res) = last_result {
            res.execution_time_ms = Some(execution_time_ms);
            Ok(res)
        } else {
            Ok(QueryResult {
                columns: Vec::new(),
                column_types: Vec::new(),
                rows: Vec::new(),
                rows_affected: Some(total_affected),
                execution_time_ms: Some(execution_time_ms),
            })
        }
    }

    async fn execute_batch(&self, sql: &str) -> DbResult<()> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;
        let mut tx = conn
            .start_transaction(mysql_async::TxOpts::default())
            .await
            .map_err(|e| DbError::query(format!("Failed to start MySQL transaction: {e}")))?;

        for trimmed in split_sql_statements(sql) {
            if trimmed.is_empty()
                || trimmed.eq_ignore_ascii_case("START TRANSACTION")
                || trimmed.eq_ignore_ascii_case("BEGIN")
                || trimmed.eq_ignore_ascii_case("COMMIT")
            {
                continue;
            }
            tx.query_drop(trimmed).await.map_err(|e| {
                DbError::query(format!("MySQL batch statement execution failed: {e}"))
            })?;
        }
        tx.commit().await.map_err(|e| {
            DbError::query(format!("Failed to commit MySQL batch transaction: {e}"))
        })?;
        Ok(())
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseSchema>> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;

        let rows: Vec<String> = conn
            .query("SHOW DATABASES;")
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

        let mut dbs = Vec::new();
        for name in rows {
            let is_sys = name == "information_schema"
                || name == "performance_schema"
                || name == "mysql"
                || name == "sys";
            dbs.push(DatabaseSchema {
                name,
                is_system: is_sys,
            });
        }
        Ok(dbs)
    }

    async fn list_tables(
        &self,
        database: Option<&str>,
        _schema: Option<&str>,
    ) -> DbResult<Vec<TableInfo>> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;

        let db_name = database.unwrap_or(&self.config.database);
        let sql = format!(
            "SELECT TABLE_NAME, TABLE_TYPE FROM information_schema.TABLES WHERE TABLE_SCHEMA = '{}' ORDER BY TABLE_NAME;",
            db_name.replace('\'', "''")
        );

        let rows: Vec<(String, String)> = conn
            .query(&sql)
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

        let mut tables = Vec::new();
        for (name, raw_type) in rows {
            tables.push(TableInfo {
                name,
                schema: Some(db_name.to_string()),
                table_type: if raw_type.contains("VIEW") {
                    "VIEW".to_string()
                } else {
                    "TABLE".to_string()
                },
                comment: None,
                row_count_estimate: None,
            });
        }
        Ok(tables)
    }

    async fn list_columns(
        &self,
        database: Option<&str>,
        _schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnInfo>> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;

        let db_name = database.unwrap_or(&self.config.database);
        let sql = format!(
            "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY, EXTRA, COLUMN_DEFAULT \
             FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION;",
            db_name.replace('\'', "''"),
            table.replace('\'', "''")
        );

        let rows: Vec<(String, String, String, String, String, Option<String>)> = conn
            .query(&sql)
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

        let mut columns = Vec::new();
        for (name, column_type, is_nullable, col_key, extra, default_val) in rows {
            columns.push(ColumnInfo {
                name,
                data_type: column_type,
                is_nullable: is_nullable.eq_ignore_ascii_case("YES"),
                is_primary_key: col_key.eq_ignore_ascii_case("PRI"),
                is_auto_increment: extra.to_lowercase().contains("auto_increment"),
                default_value: default_val,
                description: None,
            });
        }
        Ok(columns)
    }

    async fn list_indexes(
        &self,
        database: Option<&str>,
        _schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<IndexInfo>> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;

        let db_name = database.unwrap_or(&self.config.database);
        let sql = format!(
            "SELECT INDEX_NAME, NON_UNIQUE, COLUMN_NAME \
             FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY INDEX_NAME, SEQ_IN_INDEX;",
            db_name.replace('\'', "''"),
            table.replace('\'', "''")
        );

        let rows: Vec<(String, i64, String)> = conn
            .query(&sql)
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

        let mut indexes: Vec<IndexInfo> = Vec::new();
        for (name, non_unique, column_name) in rows {
            if let Some(existing) = indexes.iter_mut().find(|idx| idx.name == name) {
                existing.columns.push(column_name);
            } else {
                indexes.push(IndexInfo {
                    name: name.clone(),
                    table_name: table.to_string(),
                    columns: vec![column_name],
                    is_unique: non_unique == 0,
                    is_primary: name == "PRIMARY",
                });
            }
        }
        Ok(indexes)
    }
}

/// Safely splits a multi-statement SQL script into individual statements,
/// respecting single/double quotes, backticks, line comments, and block comments.
pub fn split_sql_statements(sql: &str) -> Vec<&str> {
    let mut stmts = Vec::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut start_idx = 0;
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        let next_b = if i + 1 < len {
            Some(bytes[i + 1])
        } else {
            None
        };

        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            if b == b'*' && next_b == Some(b'/') {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if in_single_quote {
            if b == b'\\' {
                i += 2;
                continue;
            } else if b == b'\'' {
                if next_b == Some(b'\'') {
                    i += 2;
                    continue;
                } else {
                    in_single_quote = false;
                }
            }
            i += 1;
            continue;
        }

        if in_double_quote {
            if b == b'\\' {
                i += 2;
                continue;
            } else if b == b'"' {
                if next_b == Some(b'"') {
                    i += 2;
                    continue;
                } else {
                    in_double_quote = false;
                }
            }
            i += 1;
            continue;
        }

        if in_backtick {
            if b == b'`' {
                if next_b == Some(b'`') {
                    i += 2;
                    continue;
                } else {
                    in_backtick = false;
                }
            }
            i += 1;
            continue;
        }

        if b == b'-' && next_b == Some(b'-') {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if b == b'#' {
            in_line_comment = true;
            i += 1;
            continue;
        }
        if b == b'/' && next_b == Some(b'*') {
            in_block_comment = true;
            i += 2;
            continue;
        }

        if b == b'\'' {
            in_single_quote = true;
            i += 1;
            continue;
        }
        if b == b'"' {
            in_double_quote = true;
            i += 1;
            continue;
        }
        if b == b'`' {
            in_backtick = true;
            i += 1;
            continue;
        }

        if b == b';' {
            let stmt = &sql[start_idx..i];
            if !stmt.trim().is_empty() {
                stmts.push(stmt.trim());
            }
            start_idx = i + 1;
        }

        i += 1;
    }

    if start_idx < len {
        let stmt = &sql[start_idx..];
        if !stmt.trim().is_empty() {
            stmts.push(stmt.trim());
        }
    }

    stmts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::changeset::GridChangeset;
    use crate::db::types::DatabaseFamily;

    #[tokio::test]
    async fn test_docker_mysql_connection_and_query() {
        let config = ConnectionConfig::mysql(
            "docker_mysql_test",
            "127.0.0.1",
            3306,
            "skill_up_web",
            "root",
            Some("skillup_local_test".to_string()),
        );

        let mut adapter = MysqlAdapter::new(config);

        // Attempt connection to local docker MySQL
        match adapter.connect().await {
            Ok(_) => {
                assert!(adapter.is_connected());

                // Test connection ping & server version
                let status = adapter
                    .test_connection()
                    .await
                    .expect("test_connection should succeed");
                assert!(status.connected);
                let ver = status
                    .server_version
                    .expect("server version should be present");
                assert!(
                    ver.to_lowercase().contains("mysql"),
                    "Version should contain mysql: {ver}"
                );

                // List databases
                let databases = adapter
                    .list_databases()
                    .await
                    .expect("list_databases should succeed");
                assert!(!databases.is_empty(), "Databases should not be empty");

                // Clean up any existing test table
                let _ = adapter
                    .execute_query("DROP TABLE IF EXISTS __zqlcrab_mysql_test;")
                    .await;

                // Create a test table
                let ddl = "CREATE TABLE __zqlcrab_mysql_test (
                    id INT AUTO_INCREMENT PRIMARY KEY,
                    name VARCHAR(100) NOT NULL,
                    age INT,
                    score DECIMAL(5,2),
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;";
                let ddl_res = adapter
                    .execute_query(ddl)
                    .await
                    .expect("CREATE TABLE should succeed");
                assert!(ddl_res.columns.is_empty());

                // Insert records
                let insert_sql = "INSERT INTO __zqlcrab_mysql_test (name, age, score) VALUES
                    ('Alice', 28, 95.50),
                    ('Bob', 34, 88.00),
                    ('Charlie', 22, 76.25);";
                let insert_res = adapter
                    .execute_query(insert_sql)
                    .await
                    .expect("INSERT should succeed");
                assert_eq!(insert_res.rows_affected, Some(3));

                // Query records
                let select_sql =
                    "SELECT id, name, age, score FROM __zqlcrab_mysql_test ORDER BY id ASC;";
                let select_res = adapter
                    .execute_query(select_sql)
                    .await
                    .expect("SELECT query should succeed");
                assert_eq!(select_res.columns, vec!["id", "name", "age", "score"]);
                assert_eq!(select_res.rows.len(), 3);
                assert_eq!(
                    select_res.rows[0][1],
                    QueryValue::String("Alice".to_string())
                );
                assert_eq!(select_res.rows[0][2], QueryValue::Int(28));
                assert_eq!(select_res.rows[1][1], QueryValue::String("Bob".to_string()));

                // List tables
                let tables = adapter
                    .list_tables(None, None)
                    .await
                    .expect("list_tables should succeed");
                assert!(tables.iter().any(|t| t.name == "__zqlcrab_mysql_test"));

                // List columns
                let cols = adapter
                    .list_columns(None, None, "__zqlcrab_mysql_test")
                    .await
                    .expect("list_columns should succeed");
                assert!(cols.iter().any(|c| c.name == "id" && c.is_primary_key));
                assert!(cols.iter().any(|c| c.name == "name" && !c.is_nullable));

                // Clean up test table
                let drop_res = adapter
                    .execute_query("DROP TABLE __zqlcrab_mysql_test;")
                    .await
                    .expect("DROP TABLE should succeed");
                assert!(drop_res.rows_affected.is_some());

                // Disconnect
                adapter
                    .disconnect()
                    .await
                    .expect("disconnect should succeed");
                assert!(!adapter.is_connected());
            }
            Err(e) => {
                eprintln!("Local Docker MySQL not reachable (skipped integration test): {e}");
            }
        }
    }

    #[test]
    fn test_split_sql_statements_handles_semicolons_in_literals_and_comments() {
        let sql = r#"
            -- First statement with comment
            UPDATE complex_orders SET notes = 'Line 1; contains; semicolons!' WHERE id = 1;
            /* Multi-line comment; with semicolons; */
            INSERT INTO complex_orders (notes) VALUES ("double \"quoted\" ; text");
            SELECT `col;name` FROM `tbl;name` WHERE id = 2;
        "#;

        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 3);
        assert!(stmts[0].starts_with("-- First statement"));
        assert!(stmts[0].contains("'Line 1; contains; semicolons!'"));
        assert!(stmts[1].contains(r#""double \"quoted\" ; text""#));
        assert!(stmts[2].contains("`col;name`"));
    }

    #[tokio::test]
    async fn test_mysql_complex_orders_full_scenario() {
        let config = ConnectionConfig::mysql(
            "docker_mysql_test",
            "127.0.0.1",
            3306,
            "skill_up_web",
            "root",
            Some("skillup_local_test".to_string()),
        );

        let mut adapter = MysqlAdapter::new(config);

        if let Err(e) = adapter.connect().await {
            eprintln!(
                "Skipping test_mysql_complex_orders_full_scenario (MySQL not reachable): {e}"
            );
            return;
        }

        // 1. Verify table exists
        let tables = adapter
            .list_tables(None, None)
            .await
            .expect("list_tables failed");
        assert!(
            tables.iter().any(|t| t.name == "complex_orders"),
            "complex_orders table must exist"
        );

        // 2. Verify schema columns including rich types, constraints, and auto_increment
        let columns = adapter
            .list_columns(None, None, "complex_orders")
            .await
            .expect("list_columns failed");
        assert_eq!(columns.len(), 22);

        let id_col = columns.iter().find(|c| c.name == "id").unwrap();
        assert!(id_col.is_primary_key);
        assert!(id_col.is_auto_increment);
        assert!(!id_col.is_nullable);

        let category_col = columns.iter().find(|c| c.name == "category").unwrap();
        assert!(category_col.data_type.to_lowercase().contains("enum"));

        let status_col = columns.iter().find(|c| c.name == "status").unwrap();
        assert!(status_col.data_type.to_lowercase().contains("enum"));

        let unit_price_col = columns.iter().find(|c| c.name == "unit_price").unwrap();
        assert!(
            unit_price_col
                .data_type
                .to_lowercase()
                .contains("decimal(12,4)")
        );

        let notes_col = columns.iter().find(|c| c.name == "notes").unwrap();
        assert_eq!(notes_col.data_type.to_lowercase(), "text");
        assert!(notes_col.is_nullable);

        // 3. Verify indexes including compound index idx_status_created
        let indexes = adapter
            .list_indexes(None, None, "complex_orders")
            .await
            .expect("list_indexes failed");
        assert!(
            indexes
                .iter()
                .any(|idx| idx.name == "PRIMARY" && idx.is_primary)
        );
        assert!(
            indexes
                .iter()
                .any(|idx| idx.name == "order_no" && idx.is_unique)
        );
        let compound_idx = indexes
            .iter()
            .find(|idx| idx.name == "idx_status_created")
            .expect("idx_status_created should exist");
        assert_eq!(
            compound_idx.columns,
            vec!["status".to_string(), "created_at".to_string()]
        );

        // 4. Query all 300 rows and verify data parsing (Dates, Times, JSON, Decimals, UTF8 Chinese/Emoji)
        let query_res = adapter
            .execute_query("SELECT * FROM complex_orders ORDER BY id ASC;")
            .await
            .expect("query all rows failed");
        assert_eq!(
            query_res.rows.len(),
            300,
            "Should have loaded exactly 300 rows"
        );
        assert_eq!(query_res.columns.len(), 22);

        // Verify DATE format (YYYY-MM-DD, without 00:00:00)
        let order_date_idx = query_res
            .columns
            .iter()
            .position(|c| c == "order_date")
            .unwrap();
        if let QueryValue::DateTime(dt) = &query_res.rows[0][order_date_idx] {
            assert_eq!(
                dt.len(),
                10,
                "DATE column should be YYYY-MM-DD formatted, got: {dt}"
            );
            assert!(dt.starts_with("2025-"));
        } else {
            panic!("order_date must be DateTime QueryValue");
        }

        // Verify TIME format (HH:MM:SS, no 0d prefix)
        let delivery_time_idx = query_res
            .columns
            .iter()
            .position(|c| c == "delivery_time")
            .unwrap();
        let non_null_time = query_res
            .rows
            .iter()
            .find_map(|r| match &r[delivery_time_idx] {
                QueryValue::String(s) if !s.is_empty() => Some(s),
                _ => None,
            });
        if let Some(time_str) = non_null_time {
            assert!(
                !time_str.contains("0d"),
                "Time format must not contain 0d prefix, got: {time_str}"
            );
            assert!(time_str.contains(':'));
        }

        // 5. Test complex explain plan parsing
        let explain_sql = crate::db::explain::wrap_explain_sql(
            "SELECT id, order_no, customer_name, total_amount FROM complex_orders WHERE status = 'paid' AND created_at >= '2025-01-01' ORDER BY order_date DESC LIMIT 20",
            DatabaseFamily::MySql,
        );
        let explain_res = adapter
            .execute_query(&explain_sql)
            .await
            .expect("EXPLAIN should succeed");
        let parsed_plan =
            crate::db::explain::parse_explain_result(DatabaseFamily::MySql, &explain_res);
        assert!(
            !parsed_plan.roots.is_empty(),
            "Parsed EXPLAIN plan should contain query blocks / nodes"
        );

        // 6. Test Changeset lifecycle + SQL Review Plan generation + execute_batch in MySQL transaction
        let mut cs = GridChangeset::new();

        // Staging cell updates on row 0 (id = 1)
        let target_row_idx = 0;
        let orig_name = query_res.rows[target_row_idx][query_res
            .columns
            .iter()
            .position(|c| c == "customer_name")
            .unwrap()]
        .clone();
        let orig_notes = query_res.rows[target_row_idx]
            [query_res.columns.iter().position(|c| c == "notes").unwrap()]
        .clone();

        cs.stage_cell_update(
            target_row_idx,
            query_res
                .columns
                .iter()
                .position(|c| c == "customer_name")
                .unwrap(),
            "customer_name",
            orig_name.clone(),
            QueryValue::String("极客测试员 🦀 [Auto-Modified]".to_string()),
        );

        // Update with string containing semicolons to test safe batch execution
        cs.stage_cell_update(
            target_row_idx,
            query_res.columns.iter().position(|c| c == "notes").unwrap(),
            "notes",
            orig_notes.clone(),
            QueryValue::String("Note containing; multiple; semicolons; and 'quotes'!".to_string()),
        );

        // Stage insert new row
        let mut insert_vals = vec![QueryValue::Null; 22];
        insert_vals[query_res.columns.iter().position(|c| c == "id").unwrap()] = QueryValue::Null; // auto_increment
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "order_no")
            .unwrap()] = QueryValue::String("ORD-TEST-NEW-9999".into());
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "customer_name")
            .unwrap()] = QueryValue::String("测试新客户 🚀".into());
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "category")
            .unwrap()] = QueryValue::String("enterprise".into());
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "status")
            .unwrap()] = QueryValue::String("paid".into());
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "is_vip")
            .unwrap()] = QueryValue::Bool(true);
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "item_count")
            .unwrap()] = QueryValue::Int(10);
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "unit_price")
            .unwrap()] = QueryValue::Float(199.99);
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "total_amount")
            .unwrap()] = QueryValue::Float(1999.90);
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "extra_meta")
            .unwrap()] = QueryValue::String(r#"{"test_key": "val;with;semi"}"#.into());
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "order_date")
            .unwrap()] = QueryValue::DateTime("2025-05-20".into());
        insert_vals[query_res
            .columns
            .iter()
            .position(|c| c == "delivery_time")
            .unwrap()] = QueryValue::String("18:30:00".into());
        let _temp_insert_id =
            cs.add_inserted_row(insert_vals, crate::db::changeset::InsertAnchor::default());

        assert!(cs.is_dirty());
        assert_eq!(cs.change_summary(), (2, 0, 1));

        // Generate review plan for MySQL
        let review_plan = crate::db::sql_gen::generate_review_plan(
            "complex_orders",
            None,
            DatabaseFamily::MySql,
            &columns,
            &query_res.columns,
            &query_res.rows,
            &cs,
        );

        assert_eq!(
            review_plan.statements.len(),
            2,
            "Expected 1 UPDATE and 1 INSERT"
        );
        assert!(review_plan.full_script.contains("START TRANSACTION;"));
        assert!(review_plan.full_script.contains("COMMIT;"));

        // Execute batch transaction on MySQL
        adapter
            .execute_batch(&review_plan.full_script)
            .await
            .expect("execute_batch transaction should succeed on MySQL");

        // Verify updates in MySQL
        let check_update_res = adapter
            .execute_query("SELECT customer_name, notes FROM complex_orders WHERE id = 1;")
            .await
            .expect("Check update failed");
        assert_eq!(
            check_update_res.rows[0][0],
            QueryValue::String("极客测试员 🦀 [Auto-Modified]".to_string())
        );
        assert_eq!(
            check_update_res.rows[0][1],
            QueryValue::String("Note containing; multiple; semicolons; and 'quotes'!".to_string())
        );

        // Verify insert in MySQL
        let check_insert_res = adapter.execute_query("SELECT id, customer_name, status, delivery_time FROM complex_orders WHERE order_no = 'ORD-TEST-NEW-9999';").await.expect("Check insert failed");
        assert_eq!(check_insert_res.rows.len(), 1);
        assert_eq!(
            check_insert_res.rows[0][1],
            QueryValue::String("测试新客户 🚀".to_string())
        );
        assert_eq!(
            check_insert_res.rows[0][2],
            QueryValue::String("paid".to_string())
        );
        assert_eq!(
            check_insert_res.rows[0][3],
            QueryValue::String("18:30:00".to_string())
        );

        // Clean up: restore row 1 and delete test row
        let cleanup_sql = format!(
            "UPDATE complex_orders SET customer_name = '张伟', notes = NULL WHERE id = 1; DELETE FROM complex_orders WHERE order_no = 'ORD-TEST-NEW-9999';"
        );
        adapter
            .execute_batch(&cleanup_sql)
            .await
            .expect("Cleanup failed");

        adapter.disconnect().await.expect("Disconnect failed");
    }
}
