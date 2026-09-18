//! Database error types and result definitions.

use thiserror::Error;

/// Database error enum covering connection, query execution, pool, and configuration errors.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query execution error: {0}")]
    QueryExecution(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Connection pool error: {0}")]
    PoolError(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Driver error: {0}")]
    Driver(String),
}

impl DbError {
    pub fn unsupported(op: impl Into<String>) -> Self {
        Self::UnsupportedOperation(op.into())
    }

    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    pub fn query(msg: impl Into<String>) -> Self {
        Self::QueryExecution(msg.into())
    }
}

pub type DbResult<T> = Result<T, DbError>;
