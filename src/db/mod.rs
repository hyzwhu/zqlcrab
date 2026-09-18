//! Database abstraction layer, connection pooling, and driver adapters.

pub mod adapter;
pub mod error;
pub mod types;

pub use adapter::DatabaseAdapter;
pub use error::{DbError, DbResult};
pub use types::*;
