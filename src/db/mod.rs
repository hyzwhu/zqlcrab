//! Database abstraction layer, connection pooling, and driver adapters.

pub mod adapter;
pub mod error;
pub mod handle;
pub mod manager;
pub mod mysql;
pub mod postgres;
pub mod sqlite;
pub mod types;

pub use adapter::DatabaseAdapter;
pub use error::{DbError, DbResult};
pub use handle::ActiveConnection;
pub use manager::ConnectionManager;
pub use mysql::MysqlAdapter;
pub use postgres::PostgresAdapter;
pub use sqlite::SqliteAdapter;
pub use types::*;
