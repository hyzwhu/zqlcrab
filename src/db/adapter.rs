//! Database adapter trait definition.
//!
//! Standardized interface for database operations including connection lifecycle,
//! query execution, schema reflection, and metadata inspection.

use async_trait::async_trait;
use crate::db::{
    error::DbResult,
    types::{ColumnInfo, ConnectionStatus, DatabaseSchema, IndexInfo, QueryResult, TableInfo},
};

/// Universal database adapter interface implemented by all supported database engines.
#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    /// Connect to the database.
    async fn connect(&mut self) -> DbResult<()>;

    /// Disconnect and release underlying resources.
    async fn disconnect(&mut self) -> DbResult<()>;

    /// Check if currently connected.
    fn is_connected(&self) -> bool;

    /// Test connectivity and obtain server metadata.
    async fn test_connection(&self) -> DbResult<ConnectionStatus>;

    /// Execute a SQL statement and return tabular rows or affected count.
    async fn execute_query(&self, sql: &str) -> DbResult<QueryResult>;

    /// List all databases available on the server.
    async fn list_databases(&self) -> DbResult<Vec<DatabaseSchema>> {
        Ok(Vec::new())
    }

    /// List schemas in the specified or current database.
    async fn list_schemas(&self, _database: Option<&str>) -> DbResult<Vec<String>> {
        Ok(Vec::new())
    }

    /// List tables and views in the database/schema.
    async fn list_tables(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
    ) -> DbResult<Vec<TableInfo>>;

    /// List column metadata for a given table.
    async fn list_columns(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnInfo>>;

    /// List indexes for a given table.
    async fn list_indexes(
        &self,
        _database: Option<&str>,
        _schema: Option<&str>,
        _table: &str,
    ) -> DbResult<Vec<IndexInfo>> {
        Ok(Vec::new())
    }

    /// Retrieve the CREATE TABLE DDL statement if supported.
    async fn get_table_ddl(
        &self,
        _database: Option<&str>,
        _schema: Option<&str>,
        _table: &str,
    ) -> DbResult<Option<String>> {
        Ok(None)
    }
}
