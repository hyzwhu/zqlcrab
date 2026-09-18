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
use mysql_async::{prelude::Queryable, Opts, OptsBuilder, Pool, Value};
use std::time::Instant;

pub struct MysqlAdapter {
    config: ConnectionConfig,
    pool: Option<Pool>,
}

impl MysqlAdapter {
    pub fn new(config: ConnectionConfig) -> Self {
        Self { config, pool: None }
    }

    /// Converts a mysql_async Value to uniform QueryValue.
    fn convert_value(val: Value) -> QueryValue {
        match val {
            Value::NULL => QueryValue::Null,
            Value::Bytes(b) => {
                if let Ok(s) = String::from_utf8(b.clone()) {
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

        // Verify connection
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::connection(format!("Failed to connect to MySQL: {e}")))?;

        let _ = conn
            .ping()
            .await
            .map_err(|e| DbError::connection(format!("Failed to ping MySQL: {e}")))?;

        self.pool = Some(pool);
        Ok(())
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

        let trimmed = sql.trim();
        let is_select = trimmed.len() >= 6
            && (trimmed[..6].eq_ignore_ascii_case("SELECT")
                || trimmed[..4].eq_ignore_ascii_case("SHOW")
                || trimmed[..4].eq_ignore_ascii_case("DESC")
                || trimmed[..7].eq_ignore_ascii_case("EXPLAIN"));

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

            let rows_raw = query_result
                .collect::<mysql_async::Row>()
                .await
                .map_err(|e| DbError::query(format!("Failed to collect rows: {e}")))?;

            let mut rows = Vec::with_capacity(rows_raw.len());
            for r in rows_raw {
                let mut row_vals = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let val: Value = r.get(i).unwrap_or(Value::NULL);
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
