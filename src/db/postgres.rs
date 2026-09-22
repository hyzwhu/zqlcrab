//! PostgreSQL database adapter implementation using tokio-postgres.

use crate::db::{
    adapter::DatabaseAdapter,
    error::{DbError, DbResult},
    types::{
        ColumnInfo, ConnectionConfig, ConnectionStatus, DatabaseSchema, IndexInfo, QueryResult,
        QueryValue, TableInfo,
    },
};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio_postgres::{Client, Config, NoTls, Row, types::Type};

/// Formats a tokio-postgres error, extracting detailed server diagnostics
/// (severity, message, detail, hint, where, table, column) from `as_db_error()`.
pub fn format_pg_error(e: &tokio_postgres::Error) -> String {
    if let Some(db_err) = e.as_db_error() {
        let severity = db_err.severity();
        let message = db_err.message();
        let mut msg = format!("{severity}: {message}");
        if let Some(detail) = db_err.detail() {
            msg.push_str(&format!("\nDetail: {detail}"));
        }
        if let Some(hint) = db_err.hint() {
            msg.push_str(&format!("\nHint: {hint}"));
        }
        if let Some(where_) = db_err.where_() {
            msg.push_str(&format!("\nWhere: {where_}"));
        }
        if let Some(table) = db_err.table() {
            msg.push_str(&format!("\nTable: {table}"));
        }
        if let Some(column) = db_err.column() {
            msg.push_str(&format!("\nColumn: {column}"));
        }
        msg
    } else if let Some(source) = std::error::Error::source(e) {
        format!("{e}: {source}")
    } else {
        e.to_string()
    }
}

pub struct PostgresAdapter {
    config: ConnectionConfig,
    client: Option<Arc<Mutex<Client>>>,
}

impl PostgresAdapter {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            client: None,
        }
    }

    /// Converts a PostgreSQL Row column value to QueryValue.
    fn convert_value(row: &Row, idx: usize) -> QueryValue {
        let col_type = row.columns()[idx].type_();

        if *col_type == Type::BOOL {
            if let Ok(val) = row.try_get::<_, bool>(idx) {
                return QueryValue::Bool(val);
            }
        } else if *col_type == Type::INT2 {
            if let Ok(val) = row.try_get::<_, i16>(idx) {
                return QueryValue::Int(val as i64);
            }
        } else if *col_type == Type::INT4 {
            if let Ok(val) = row.try_get::<_, i32>(idx) {
                return QueryValue::Int(val as i64);
            }
        } else if *col_type == Type::INT8 {
            if let Ok(val) = row.try_get::<_, i64>(idx) {
                return QueryValue::Int(val);
            }
        } else if *col_type == Type::FLOAT4 {
            if let Ok(val) = row.try_get::<_, f32>(idx) {
                return QueryValue::Float(val as f64);
            }
        } else if *col_type == Type::FLOAT8 {
            if let Ok(val) = row.try_get::<_, f64>(idx) {
                return QueryValue::Float(val);
            }
        } else if *col_type == Type::BYTEA {
            if let Ok(val) = row.try_get::<_, Vec<u8>>(idx) {
                return QueryValue::Bytes(val);
            }
        } else if *col_type == Type::TEXT
            || *col_type == Type::VARCHAR
            || *col_type == Type::BPCHAR
            || *col_type == Type::NAME
        {
            if let Ok(val) = row.try_get::<_, String>(idx) {
                return QueryValue::String(val);
            }
        } else if *col_type == Type::JSON || *col_type == Type::JSONB {
            if let Ok(val) = row.try_get::<_, serde_json::Value>(idx) {
                return QueryValue::String(val.to_string());
            }
            if let Ok(val) = row.try_get::<_, String>(idx) {
                return QueryValue::String(val);
            }
        }

        // Generic fallback to text / string representation
        if let Ok(s) = row.try_get::<_, String>(idx) {
            QueryValue::String(s)
        } else if let Ok(val) = row.try_get::<_, serde_json::Value>(idx) {
            QueryValue::String(val.to_string())
        } else {
            QueryValue::Null
        }
    }
}

#[async_trait]
impl DatabaseAdapter for PostgresAdapter {
    async fn connect(&mut self) -> DbResult<()> {
        let mut pg_config = Config::new();
        pg_config.host(&self.config.host);
        if self.config.port > 0 {
            pg_config.port(self.config.port);
        }
        if !self.config.database.is_empty() {
            pg_config.dbname(&self.config.database);
        }
        if !self.config.username.is_empty() {
            pg_config.user(&self.config.username);
        }
        if let Some(ref pwd) = self.config.password {
            pg_config.password(pwd);
        }
        pg_config.connect_timeout(std::time::Duration::from_secs(
            self.config.connect_timeout_secs,
        ));

        let (client, connection) = pg_config
            .connect(NoTls)
            .await
            .map_err(|e| DbError::connection(format!("Failed to connect to PostgreSQL: {e}")))?;

        // Spawn connection poller on tokio runtime
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error: {e}");
            }
        });

        self.client = Some(Arc::new(Mutex::new(client)));
        Ok(())
    }

    async fn disconnect(&mut self) -> DbResult<()> {
        self.client = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn test_connection(&self) -> DbResult<ConnectionStatus> {
        let start = Instant::now();
        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let row = client
            .query_one("SELECT version(), current_database();", &[])
            .await
            .map_err(|e| DbError::query(format!("PostgreSQL health check failed: {}", format_pg_error(&e))))?;

        let version: String = row.get(0);
        let curr_db: String = row.get(1);
        let ping_ms = start.elapsed().as_millis() as u64;

        Ok(ConnectionStatus {
            connected: true,
            server_version: Some(version),
            current_database: Some(curr_db),
            ping_ms: Some(ping_ms),
        })
    }

    async fn execute_query(&self, sql: &str) -> DbResult<QueryResult> {
        let start = Instant::now();
        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

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

        for (idx, stmt) in statements.iter().enumerate() {
            let is_select = crate::db::safety::QuerySafetyValidator::is_result_set_query(stmt);
            let snippet = crate::db::sql_gen::truncate_sql_snippet(stmt, 60);

            if is_select {
                let rows = client
                    .query(stmt.as_str(), &[])
                    .await
                    .map_err(|e| {
                        let err_desc = format_pg_error(&e);
                        if total_stmts > 1 {
                            DbError::query(format!(
                                "Statement {}/{} failed [{}]:\n{}",
                                idx + 1,
                                total_stmts,
                                snippet,
                                err_desc
                            ))
                        } else {
                            DbError::query(format!("Query failed: {err_desc}"))
                        }
                    })?;

                let columns: Vec<String> = if let Some(first) = rows.first() {
                    first
                        .columns()
                        .iter()
                        .map(|c| c.name().to_string())
                        .collect()
                } else {
                    Vec::new()
                };

                let column_types: Vec<String> = if let Some(first) = rows.first() {
                    first
                        .columns()
                        .iter()
                        .map(|c| c.type_().name().to_string())
                        .collect()
                } else {
                    Vec::new()
                };

                let mut result_rows = Vec::with_capacity(rows.len());
                for row in &rows {
                    let mut row_vals = Vec::with_capacity(columns.len());
                    for i in 0..columns.len() {
                        row_vals.push(Self::convert_value(row, i));
                    }
                    result_rows.push(row_vals);
                }

                last_result = Some(QueryResult {
                    columns,
                    column_types,
                    rows: result_rows,
                    rows_affected: None,
                    execution_time_ms: None,
                });
            } else {
                let affected = client
                    .execute(stmt.as_str(), &[])
                    .await
                    .map_err(|e| {
                        let err_desc = format_pg_error(&e);
                        if total_stmts > 1 {
                            DbError::query(format!(
                                "Statement {}/{} failed [{}]:\n{}",
                                idx + 1,
                                total_stmts,
                                snippet,
                                err_desc
                            ))
                        } else {
                            DbError::query(format!("Execution failed: {err_desc}"))
                        }
                    })?;
                total_affected += affected;
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
        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;
        client
            .batch_execute(sql)
            .await
            .map_err(|e| DbError::query(format!("PostgreSQL batch execution failed: {}", format_pg_error(&e))))?;
        Ok(())
    }

    async fn list_databases(&self) -> DbResult<Vec<DatabaseSchema>> {
        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let sql = "SELECT datname, datistemplate FROM pg_database WHERE datallowconn = true ORDER BY datname;";
        let rows = client
            .query(sql, &[])
            .await
            .map_err(|e| DbError::query(format_pg_error(&e)))?;

        let mut dbs = Vec::new();
        for row in rows {
            let name: String = row.get(0);
            let is_template: bool = row.get(1);
            dbs.push(DatabaseSchema {
                name,
                is_system: is_template,
            });
        }
        Ok(dbs)
    }

    async fn list_schemas(&self, _database: Option<&str>) -> DbResult<Vec<String>> {
        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let sql = "SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT LIKE 'pg_%' AND schema_name != 'information_schema' ORDER BY schema_name;";
        let rows = client
            .query(sql, &[])
            .await
            .map_err(|e| DbError::query(format_pg_error(&e)))?;

        let mut schemas = Vec::new();
        for row in rows {
            let name: String = row.get(0);
            schemas.push(name);
        }
        if schemas.is_empty() {
            schemas.push("public".to_string());
        }
        Ok(schemas)
    }

    async fn list_tables(
        &self,
        _database: Option<&str>,
        schema: Option<&str>,
    ) -> DbResult<Vec<TableInfo>> {
        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let rows = if let Some(schema_name) = schema {
            client
                .query(
                    "SELECT table_schema, table_name, table_type FROM information_schema.tables WHERE table_schema = $1 ORDER BY table_name;",
                    &[&schema_name],
                )
                .await
        } else {
            client
                .query(
                    "SELECT table_schema, table_name, table_type FROM information_schema.tables WHERE table_schema NOT LIKE 'pg_%' AND table_schema != 'information_schema' ORDER BY table_schema, table_name;",
                    &[],
                )
                .await
        }
        .map_err(|e| DbError::query(format_pg_error(&e)))?;

        let mut tables = Vec::new();
        for row in rows {
            let schema_name: String = row.get(0);
            let name: String = row.get(1);
            let raw_type: String = row.get(2);
            tables.push(TableInfo {
                name,
                schema: Some(schema_name),
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
        _database: Option<&str>,
        schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnInfo>> {
        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let schema_name = schema.unwrap_or("public");
        let sql = "SELECT column_name, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2 ORDER BY ordinal_position;";
        let rows = client
            .query(sql, &[&schema_name, &table])
            .await
            .map_err(|e| DbError::query(format_pg_error(&e)))?;

        let mut columns = Vec::new();
        for row in rows {
            let name: String = row.get(0);
            let data_type: String = row.get(1);
            let nullable_str: String = row.get(2);
            let default_val: Option<String> = row.get(3);

            columns.push(ColumnInfo {
                name,
                data_type,
                is_nullable: nullable_str.eq_ignore_ascii_case("YES"),
                is_primary_key: false,
                is_auto_increment: default_val
                    .as_deref()
                    .map(|d| d.contains("nextval"))
                    .unwrap_or(false),
                default_value: default_val,
                description: None,
            });
        }
        Ok(columns)
    }

    async fn list_indexes(
        &self,
        _database: Option<&str>,
        schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<IndexInfo>> {
        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let schema_name = schema.unwrap_or("public");
        let sql = "SELECT indexname FROM pg_indexes WHERE schemaname = $1 AND tablename = $2;";
        let rows = client
            .query(sql, &[&schema_name, &table])
            .await
            .map_err(|e| DbError::query(format_pg_error(&e)))?;

        let mut indexes = Vec::new();
        for row in rows {
            let name: String = row.get(0);
            indexes.push(IndexInfo {
                name,
                table_name: table.to_string(),
                columns: Vec::new(),
                is_unique: false,
                is_primary: false,
            });
        }
        Ok(indexes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_adapter_initial_state() {
        let config = ConnectionConfig::postgres("test_pg", "localhost", 5432, "testdb", "postgres", None);
        let adapter = PostgresAdapter::new(config);
        assert!(!adapter.is_connected());
    }
}
