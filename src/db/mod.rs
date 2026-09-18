//! Database abstraction layer, connection pooling, and driver adapters.

pub mod adapter;
pub mod error;
pub mod explain;
pub mod export;
pub mod handle;
pub mod history;
pub mod manager;
pub mod mysql;
pub mod postgres;
pub mod runtime;
pub mod safety;
pub mod sqlite;
pub mod sql_format;
pub mod types;

pub use adapter::DatabaseAdapter;
pub use error::{DbError, DbResult};
pub use export::{export_result, ExportFormat, ExportOptions};
pub use handle::ActiveConnection;
pub use history::{QueryHistoryItem, QueryHistoryManager, QueryHistoryStatus};
pub use manager::ConnectionManager;
pub use mysql::MysqlAdapter;
pub use postgres::PostgresAdapter;
pub use runtime::{run_on_tokio, tokio_runtime};
pub use safety::QuerySafetyValidator;
pub use sqlite::SqliteAdapter;
pub use types::*;
