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
use mysql_async::{consts::ColumnType, prelude::Queryable, Opts, OptsBuilder, Pool, Value};
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
                QueryValue::DateTime(format!("{y:04}-{m:02}-{d:02} {h:02}:{i:02}:{s:02}"))
            }
            Value::Time(is_neg, d, h, m, s, _u) => {
                let sign = if is_neg { "-" } else { "" };
                QueryValue::String(format!("{sign}{d}d {h:02}:{m:02}:{s:02}"))
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
                self.config.host, self.config.port, timeout_dur.as_secs()
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
        let pool = self.pool.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
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
        let pool = self.pool.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(e.to_string()))?;

        let is_select = crate::db::safety::QuerySafetyValidator::is_result_set_query(sql);

        if is_select {
            let mut query_result = conn
                .query_iter(sql)
                .await
                .map_err(|e| DbError::query(format!("Query failed: {e}")))?;

            let columns: Vec<String> = query_result
                .columns()
                .map(|cols| cols.iter().map(|c| c.name_str().to_string()).collect())
                .unwrap_or_default();
            let col_count = columns.len();
            let column_types: Vec<String> = query_result
                .columns()
                .map(|cols| cols.iter().map(|c| format!("{:?}", c.column_type())).collect())
                .unwrap_or_default();

            let col_types: Vec<ColumnType> = query_result
                .columns()
                .map(|cols| cols.iter().map(|c| c.column_type()).collect())
                .unwrap_or_default();

            let rows_raw = query_result
                .collect::<mysql_async::Row>()
                .await
                .map_err(|e| DbError::query(format!("Failed to collect rows: {e}")))?;

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

            let execution_time_ms = start.elapsed().as_millis() as u64;
            Ok(QueryResult {
                columns,
                column_types,
                rows,
                rows_affected: None,
                execution_time_ms: Some(execution_time_ms),
            })
        } else {
            conn.query_drop(sql)
                .await
                .map_err(|e| DbError::query(format!("Statement execution failed: {e}")))?;
            let affected = conn.affected_rows();
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
        let pool = self.pool.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool.get_conn().await.map_err(|e| DbError::connection(e.to_string()))?;
        let mut tx = conn
            .start_transaction(mysql_async::TxOpts::default())
            .await
            .map_err(|e| DbError::query(format!("Failed to start MySQL transaction: {e}")))?;

        for stmt in sql.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty()
                || trimmed.eq_ignore_ascii_case("START TRANSACTION")
                || trimmed.eq_ignore_ascii_case("BEGIN")
                || trimmed.eq_ignore_ascii_case("COMMIT")
            {
                continue;
            }
            tx.query_drop(trimmed)
                .await
                .map_err(|e| DbError::query(format!("MySQL batch statement execution failed: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| DbError::query(format!("Failed to commit MySQL batch transaction: {e}")))?;
        Ok(())
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseSchema>> {
        let pool = self.pool.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool.get_conn().await.map_err(|e| DbError::connection(e.to_string()))?;

        let rows: Vec<String> = conn
            .query("SHOW DATABASES;")
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

        let mut dbs = Vec::new();
        for name in rows {
            let is_sys = name == "information_schema" || name == "performance_schema" || name == "mysql" || name == "sys";
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
        let pool = self.pool.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool.get_conn().await.map_err(|e| DbError::connection(e.to_string()))?;

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
        let pool = self.pool.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool.get_conn().await.map_err(|e| DbError::connection(e.to_string()))?;

        let db_name = database.unwrap_or(&self.config.database);
        let sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_KEY, EXTRA, COLUMN_DEFAULT \
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
        for (name, data_type, is_nullable, col_key, extra, default_val) in rows {
            columns.push(ColumnInfo {
                name,
                data_type,
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
        let pool = self.pool.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let mut conn = pool.get_conn().await.map_err(|e| DbError::connection(e.to_string()))?;

        let db_name = database.unwrap_or(&self.config.database);
        let sql = format!(
            "SELECT INDEX_NAME, NON_UNIQUE FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' GROUP BY INDEX_NAME, NON_UNIQUE;",
            db_name.replace('\'', "''"),
            table.replace('\'', "''")
        );

        let rows: Vec<(String, i32)> = conn
            .query(&sql)
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

        let mut indexes = Vec::new();
        for (name, non_unique) in rows {
            indexes.push(IndexInfo {
                name: name.clone(),
                table_name: table.to_string(),
                columns: Vec::new(),
                is_unique: non_unique == 0,
                is_primary: name == "PRIMARY",
            });
        }
        Ok(indexes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                let status = adapter.test_connection().await.expect("test_connection should succeed");
                assert!(status.connected);
                let ver = status.server_version.expect("server version should be present");
                assert!(ver.to_lowercase().contains("mysql"), "Version should contain mysql: {ver}");

                // List databases
                let databases = adapter.list_databases().await.expect("list_databases should succeed");
                assert!(!databases.is_empty(), "Databases should not be empty");

                // Clean up any existing test table
                let _ = adapter.execute_query("DROP TABLE IF EXISTS __zqlcrab_mysql_test;").await;

                // Create a test table
                let ddl = "CREATE TABLE __zqlcrab_mysql_test (
                    id INT AUTO_INCREMENT PRIMARY KEY,
                    name VARCHAR(100) NOT NULL,
                    age INT,
                    score DECIMAL(5,2),
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;";
                let ddl_res = adapter.execute_query(ddl).await.expect("CREATE TABLE should succeed");
                assert!(ddl_res.columns.is_empty());

                // Insert records
                let insert_sql = "INSERT INTO __zqlcrab_mysql_test (name, age, score) VALUES
                    ('Alice', 28, 95.50),
                    ('Bob', 34, 88.00),
                    ('Charlie', 22, 76.25);";
                let insert_res = adapter.execute_query(insert_sql).await.expect("INSERT should succeed");
                assert_eq!(insert_res.rows_affected, Some(3));

                // Query records
                let select_sql = "SELECT id, name, age, score FROM __zqlcrab_mysql_test ORDER BY id ASC;";
                let select_res = adapter.execute_query(select_sql).await.expect("SELECT query should succeed");
                assert_eq!(select_res.columns, vec!["id", "name", "age", "score"]);
                assert_eq!(select_res.rows.len(), 3);
                assert_eq!(select_res.rows[0][1], QueryValue::String("Alice".to_string()));
                assert_eq!(select_res.rows[0][2], QueryValue::Int(28));
                assert_eq!(select_res.rows[1][1], QueryValue::String("Bob".to_string()));

                // List tables
                let tables = adapter.list_tables(None, None).await.expect("list_tables should succeed");
                assert!(tables.iter().any(|t| t.name == "__zqlcrab_mysql_test"));

                // List columns
                let cols = adapter.list_columns(None, None, "__zqlcrab_mysql_test").await.expect("list_columns should succeed");
                assert!(cols.iter().any(|c| c.name == "id" && c.is_primary_key));
                assert!(cols.iter().any(|c| c.name == "name" && !c.is_nullable));

                // Clean up test table
                let drop_res = adapter.execute_query("DROP TABLE __zqlcrab_mysql_test;").await.expect("DROP TABLE should succeed");
                assert!(drop_res.rows_affected.is_some());

                // Disconnect
                adapter.disconnect().await.expect("disconnect should succeed");
                assert!(!adapter.is_connected());
            }
            Err(e) => {
                eprintln!("Local Docker MySQL not reachable (skipped integration test): {e}");
            }
        }
    }
}
