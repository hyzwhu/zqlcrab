//! Database abstraction layer, connection pooling, and driver adapters.

pub mod adapter;
pub mod autocomplete;
pub mod changeset;
pub mod error;
pub mod explain;
pub mod export;
pub mod handle;
pub mod history;
pub mod import;
pub mod manager;
pub mod mock_data;
pub mod mysql;
pub mod postgres;
pub mod remote_adapter;
pub mod runtime;
pub mod safety;
pub mod sql_format;
pub mod sql_gen;
pub mod sqlite;
pub mod types;

pub use adapter::DatabaseAdapter;
pub use autocomplete::{SqlCompletionProvider, SqlMetadataCache};
pub use changeset::{CellEdit, GridChangeset, RowDeletion};
pub use error::{DbError, DbResult};
pub use export::{ExportFormat, ExportOptions, export_result};
pub use handle::ActiveConnection;
pub use history::{QueryHistoryItem, QueryHistoryManager, QueryHistoryStatus};
pub use manager::ConnectionManager;
pub use mock_data::{
    MockColumnConfig, MockGeneratorType, MockProgress, MockResult, execute_mock_seeding,
    generate_batch_insert_sql, generate_mock_preview, infer_mock_generator,
    initialize_column_configs,
};
pub use mysql::MysqlAdapter;
pub use postgres::PostgresAdapter;
pub use remote_adapter::RemoteHttpAdapter;
pub use runtime::{run_on_tokio, tokio_runtime};
pub use safety::QuerySafetyValidator;
pub use sql_gen::{
    ColumnDef, CreateTableDef, SqlReviewPlan, dialect_data_types, dialect_presets,
    generate_create_table_sql, generate_review_plan, split_sql_statements, truncate_sql_snippet,
};
pub use sqlite::SqliteAdapter;
pub use types::*;
