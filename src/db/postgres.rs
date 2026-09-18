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
use tokio_postgres::{types::Type, Client, Config, NoTls, Row};

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
        } else if *col_type == Type::TEXT || *col_type == Type::VARCHAR || *col_type == Type::BPCHAR || *col_type == Type::NAME {
            if let Ok(val) = row.try_get::<_, String>(idx) {
                return QueryValue::String(val);
            }
        }

        // Generic fallback to text / string representation
        if let Ok(s) = row.try_get::<_, String>(idx) {
            QueryValue::String(s)
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
        pg_config.connect_timeout(std::time::Duration::from_secs(self.config.connect_timeout_secs));

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
        let client_arc = self.client.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let row = client
            .query_one("SELECT version(), current_database();", &[])
            .await
            .map_err(|e| DbError::query(format!("PostgreSQL health check failed: {e}")))?;

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
        let client_arc = self.client.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let is_select = crate::db::safety::QuerySafetyValidator::is_result_set_query(sql);

        if is_select {
            let rows = client
                .query(sql, &[])
                .await
                .map_err(|e| DbError::query(format!("Query failed: {e}")))?;

            let columns: Vec<String> = if let Some(first) = rows.first() {
                first.columns().iter().map(|c| c.name().to_string()).collect()
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

            let execution_time_ms = start.elapsed().as_millis() as u64;
            Ok(QueryResult {
                columns,
                column_types,
                rows: result_rows,
                rows_affected: None,
                execution_time_ms: Some(execution_time_ms),
            })
        } else {
            let affected = client
                .execute(sql, &[])
                .await
                .map_err(|e| DbError::query(format!("Execution failed: {e}")))?;
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
        let client_arc = self.client.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let sql = "SELECT datname, datistemplate FROM pg_database WHERE datallowconn = true ORDER BY datname;";
        let rows = client
            .query(sql, &[])
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

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
        let client_arc = self.client.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let sql = "SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT LIKE 'pg_%' AND schema_name != 'information_schema' ORDER BY schema_name;";
        let rows = client
            .query(sql, &[])
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

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
        let client_arc = self.client.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let schema_name = schema.unwrap_or("public");
        let sql = "SELECT table_name, table_type FROM information_schema.tables WHERE table_schema = $1 ORDER BY table_name;";
        let rows = client
            .query(sql, &[&schema_name])
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

        let mut tables = Vec::new();
        for row in rows {
            let name: String = row.get(0);
            let raw_type: String = row.get(1);
            tables.push(TableInfo {
                name,
                schema: Some(schema_name.to_string()),
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
        let client_arc = self.client.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let schema_name = schema.unwrap_or("public");
        let sql = "SELECT column_name, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2 ORDER BY ordinal_position;";
        let rows = client
            .query(sql, &[&schema_name, &table])
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

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
        let client_arc = self.client.as_ref().ok_or_else(|| DbError::connection("Not connected"))?;
        let client = client_arc.lock().await;

        let schema_name = schema.unwrap_or("public");
        let sql = "SELECT indexname FROM pg_indexes WHERE schemaname = $1 AND tablename = $2;";
        let rows = client
            .query(sql, &[&schema_name, &table])
            .await
            .map_err(|e| DbError::query(e.to_string()))?;

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
