//! Unified active connection handle.

use crate::db::{
    adapter::DatabaseAdapter,
    error::DbResult,
    mysql::MysqlAdapter,
    postgres::PostgresAdapter,
    sqlite::SqliteAdapter,
    types::{
        ColumnInfo, ConnectionConfig, ConnectionStatus, DatabaseSchema, DatabaseType, IndexInfo,
        QueryResult, TableInfo,
    },
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// An active, established database connection handle wrapping the underlying engine adapter.
#[derive(Clone)]
pub struct ActiveConnection {
    pub id: String,
    pub config: ConnectionConfig,
    pub adapter: Arc<Mutex<Box<dyn DatabaseAdapter>>>,
    pub status: Option<ConnectionStatus>,
}

impl ActiveConnection {
    /// Instantiates a new active connection from a configuration profile.
    pub fn new(config: ConnectionConfig) -> Self {
        let id = config.id.clone();
        let boxed_adapter: Box<dyn DatabaseAdapter> = match config.db_type {
            DatabaseType::Sqlite => Box::new(SqliteAdapter::new(config.clone())),
            DatabaseType::Postgres => Box::new(PostgresAdapter::new(config.clone())),
            DatabaseType::Mysql => Box::new(MysqlAdapter::new(config.clone())),
        };

        Self {
            id,
            config,
            adapter: Arc::new(Mutex::new(boxed_adapter)),
            status: None,
        }
    }

    /// Establishes and tests a connection for a given configuration.
    pub async fn connect_config(config: ConnectionConfig) -> DbResult<Self> {
        let conn = Self::new(config);
        conn.adapter.lock().await.connect().await?;
        let status = conn.adapter.lock().await.test_connection().await.ok();
        Ok(Self {
            status,
            ..conn
        })
    }

    /// Tests a configuration without keeping an active handle open.
    pub async fn test_config(config: &ConnectionConfig) -> DbResult<ConnectionStatus> {
        let conn = Self::new(config.clone());
        conn.adapter.lock().await.connect().await?;
        let status = conn.adapter.lock().await.test_connection().await;
        let _ = conn.adapter.lock().await.disconnect().await;
        status
    }

    pub async fn connect(&self) -> DbResult<()> {
        let mut adapter = self.adapter.lock().await;
        adapter.connect().await
    }

    pub async fn disconnect(&self) -> DbResult<()> {
        let mut adapter = self.adapter.lock().await;
        adapter.disconnect().await
    }

    pub async fn is_connected(&self) -> bool {
        let adapter = self.adapter.lock().await;
        adapter.is_connected()
    }

    pub async fn test_connection(&self) -> DbResult<ConnectionStatus> {
        let adapter = self.adapter.lock().await;
        adapter.test_connection().await
    }

    pub async fn execute_query(&self, sql: &str) -> DbResult<QueryResult> {
        crate::db::safety::QuerySafetyValidator::validate_query(sql, self.config.is_read_only)?;
        let adapter = self.adapter.lock().await;
        adapter.execute_query(sql).await
    }

    pub async fn list_databases(&self) -> DbResult<Vec<DatabaseSchema>> {
        let adapter = self.adapter.lock().await;
        adapter.list_databases().await
    }

    pub async fn list_schemas(&self, database: Option<&str>) -> DbResult<Vec<String>> {
        let adapter = self.adapter.lock().await;
        adapter.list_schemas(database).await
    }

    pub async fn list_tables(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
    ) -> DbResult<Vec<TableInfo>> {
        let adapter = self.adapter.lock().await;
        adapter.list_tables(database, schema).await
    }

    pub async fn list_columns(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnInfo>> {
        let adapter = self.adapter.lock().await;
        adapter.list_columns(database, schema, table).await
    }

    pub async fn list_indexes(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<IndexInfo>> {
        let adapter = self.adapter.lock().await;
        adapter.list_indexes(database, schema, table).await
    }

    pub async fn get_table_ddl(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
        table: &str,
    ) -> DbResult<Option<String>> {
        let adapter = self.adapter.lock().await;
        adapter.get_table_ddl(database, schema, table).await
    }
}
