//! Data import engine supporting CSV, TSV, and SQL files with automatic parameter inference,
//! smart column mapping, batch chunk insertion, and error isolation.

pub mod column_mapper;
pub mod csv_sniffer;
pub mod executor;

pub use column_mapper::{ColumnMapping, auto_map_columns};
pub use csv_sniffer::{
    CsvDelimiter, CsvPreviewData, CsvSniffer, FileEncoding, ImportFormat, SqlPreviewData,
};
pub use executor::{
    CsvImportConfig, ErrorPolicy, ImportErrorRow, ImportExecutor, ImportProgress, ImportResult,
    format_csv_value_for_sql,
};
