//! Data export serializers for converting query results into CSV, TSV, JSON, NDJSON, Markdown, and SQL statements.

use crate::db::types::{DatabaseFamily, QueryResult, QueryValue, quote_ident};
use serde::{Deserialize, Serialize};

/// Supported export target formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    #[default]
    SqlDump,
    SqlInsert,
    Csv,
    Tsv,
    Json,
    Ndjson,
    Markdown,
}

impl ExportFormat {
    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::SqlDump => "SQL Dump (DDL + Data)",
            Self::SqlInsert => "SQL Inserts",
            Self::Csv => "CSV (Comma Separated)",
            Self::Tsv => "TSV (Tab Separated)",
            Self::Json => "JSON (Array)",
            Self::Ndjson => "NDJSON (Stream)",
            Self::Markdown => "Markdown (Table)",
        }
    }

    /// Suggested file extension for saving.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::SqlDump | Self::SqlInsert => "sql",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Json => "json",
            Self::Ndjson => "json",
            Self::Markdown => "md",
        }
    }
}

/// Scope of table export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExportScope {
    #[default]
    SchemaAndData,
    DataOnly,
    SchemaOnly,
}

impl ExportScope {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::SchemaAndData => "Structure & Data",
            Self::DataOnly => "Data Only",
            Self::SchemaOnly => "Structure Only (DDL)",
        }
    }
}

/// Configuration options for SQL dumps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlDumpOptions {
    pub drop_table_if_exists: bool,
    pub wrap_in_transaction: bool,
    pub batch_size: usize,
    pub include_comments: bool,
}

impl Default for SqlDumpOptions {
    fn default() -> Self {
        Self {
            drop_table_if_exists: true,
            wrap_in_transaction: true,
            batch_size: 100,
            include_comments: true,
        }
    }
}

/// Configuration options for Delimited files (CSV / TSV).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvOptions {
    pub delimiter: char,
    pub include_headers: bool,
    pub null_representation: String,
    pub quote_char: char,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: ',',
            include_headers: true,
            null_representation: String::new(),
            quote_char: '"',
        }
    }
}

/// Configuration options for JSON exports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonOptions {
    pub pretty: bool,
    pub ndjson: bool,
}

impl Default for JsonOptions {
    fn default() -> Self {
        Self {
            pretty: true,
            ndjson: false,
        }
    }
}

/// Comprehensive table dump and export configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableDumpConfig {
    pub table_name: String,
    pub scope: ExportScope,
    pub format: ExportFormat,
    pub sql_options: SqlDumpOptions,
    pub csv_options: CsvOptions,
    pub json_options: JsonOptions,
    pub family: DatabaseFamily,
}

impl Default for TableDumpConfig {
    fn default() -> Self {
        Self {
            table_name: String::new(),
            scope: ExportScope::SchemaAndData,
            format: ExportFormat::SqlDump,
            sql_options: SqlDumpOptions::default(),
            csv_options: CsvOptions::default(),
            json_options: JsonOptions::default(),
            family: DatabaseFamily::Sqlite,
        }
    }
}

/// Legacy/quick export options for query result grids.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub include_headers: bool,
    pub pretty_json: bool,
    pub table_name: Option<String>,
    pub batch_size: usize,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            include_headers: true,
            pretty_json: true,
            table_name: None,
            batch_size: 100,
        }
    }
}

/// Export a query result based on legacy options (backward compatible).
pub fn export_result(result: &QueryResult, options: &ExportOptions) -> String {
    match options.format {
        ExportFormat::Csv => export_csv(result, options.include_headers),
        ExportFormat::Tsv => {
            let mut opts = CsvOptions::default();
            opts.delimiter = '\t';
            opts.include_headers = options.include_headers;
            export_csv_with_options(result, &opts)
        }
        ExportFormat::Json => export_json(result, options.pretty_json),
        ExportFormat::Ndjson => export_ndjson(result),
        ExportFormat::Markdown => export_markdown(result),
        ExportFormat::SqlInsert | ExportFormat::SqlDump => {
            let tbl = options.table_name.as_deref().unwrap_or("exported_table");
            export_sql_inserts(result, tbl, options.batch_size)
        }
    }
}

/// Generate a complete table dump (DDL, Data, or both) or format conversion.
pub fn generate_table_dump(
    ddl: Option<&str>,
    data: Option<&QueryResult>,
    config: &TableDumpConfig,
) -> String {
    match config.format {
        ExportFormat::SqlDump | ExportFormat::SqlInsert => generate_sql_dump(ddl, data, config),
        ExportFormat::Csv => {
            if let Some(res) = data {
                export_csv_with_options(res, &config.csv_options)
            } else {
                String::new()
            }
        }
        ExportFormat::Tsv => {
            if let Some(res) = data {
                let mut opts = config.csv_options.clone();
                opts.delimiter = '\t';
                export_csv_with_options(res, &opts)
            } else {
                String::new()
            }
        }
        ExportFormat::Json => {
            if let Some(res) = data {
                export_json(res, config.json_options.pretty)
            } else {
                "[]".to_string()
            }
        }
        ExportFormat::Ndjson => {
            if let Some(res) = data {
                export_ndjson(res)
            } else {
                String::new()
            }
        }
        ExportFormat::Markdown => {
            if let Some(res) = data {
                export_markdown(res)
            } else {
                String::new()
            }
        }
    }
}

/// Generate a preview with limited rows to avoid heavy memory allocation.
pub fn generate_export_preview(
    ddl: Option<&str>,
    data: Option<&QueryResult>,
    config: &TableDumpConfig,
    max_rows: usize,
) -> String {
    let sliced_data = data.map(|res| {
        if res.rows.len() <= max_rows {
            res.clone()
        } else {
            let mut sliced = res.clone();
            sliced.rows.truncate(max_rows);
            sliced
        }
    });

    let mut preview = generate_table_dump(ddl, sliced_data.as_ref(), config);

    if let Some(res) = data {
        if res.rows.len() > max_rows {
            let omitted = res.rows.len() - max_rows;
            match config.format {
                ExportFormat::SqlDump | ExportFormat::SqlInsert => {
                    preview.push_str(&format!(
                        "\n-- ... [Preview truncated: {omitted} more row(s) omitted in preview] ...\n"
                    ));
                }
                ExportFormat::Csv | ExportFormat::Tsv => {
                    preview.push_str(&format!(
                        "\n# ... [Preview truncated: {omitted} more row(s) omitted in preview] ...\n"
                    ));
                }
                ExportFormat::Markdown => {
                    preview.push_str(&format!(
                        "\n*... [Preview truncated: {omitted} more row(s) omitted in preview] ...*\n"
                    ));
                }
                ExportFormat::Json => {
                    if preview.ends_with(']') {
                        preview.pop();
                        preview.push_str(&format!(
                            "  // ... [Preview truncated: {omitted} more object(s) omitted in preview]\n]"
                        ));
                    }
                }
                ExportFormat::Ndjson => {
                    preview.push_str(&format!(
                        "// ... [Preview truncated: {omitted} more line(s) omitted in preview]\n"
                    ));
                }
            }
        }
    }

    preview
}

fn generate_sql_dump(
    ddl: Option<&str>,
    data: Option<&QueryResult>,
    config: &TableDumpConfig,
) -> String {
    let mut out = String::new();
    let quoted_table = quote_ident(&config.table_name, config.family);

    // 1. Header Comments
    if config.sql_options.include_comments {
        out.push_str("-- -------------------------------------------------------------\n");
        out.push_str(&format!("-- Table Dump: {}\n", config.table_name));
        out.push_str(&format!("-- Database Engine: {:?}\n", config.family));
        out.push_str(&format!("-- Scope: {}\n", config.scope.display_name()));
        out.push_str("-- Generated by: zqlcrab\n");
        out.push_str("-- -------------------------------------------------------------\n\n");
    }

    // 2. Transaction Start
    if config.sql_options.wrap_in_transaction {
        match config.family {
            DatabaseFamily::Sqlite => out.push_str("BEGIN TRANSACTION;\n\n"),
            DatabaseFamily::Postgres => out.push_str("BEGIN;\n\n"),
            DatabaseFamily::MySql => out.push_str("START TRANSACTION;\n\n"),
        }
    }

    // 3. Schema DDL
    if config.scope == ExportScope::SchemaAndData || config.scope == ExportScope::SchemaOnly {
        if config.sql_options.drop_table_if_exists {
            let drop_sql = match config.family {
                DatabaseFamily::Postgres => {
                    format!("DROP TABLE IF EXISTS {quoted_table} CASCADE;\n\n")
                }
                _ => format!("DROP TABLE IF EXISTS {quoted_table};\n\n"),
            };
            out.push_str(&drop_sql);
        }

        if let Some(ddl_text) = ddl {
            let trimmed = ddl_text.trim();
            if !trimmed.is_empty() {
                out.push_str(trimmed);
                if !trimmed.ends_with(';') {
                    out.push(';');
                }
                out.push_str("\n\n");
            }
        }
    }

    // 4. Data Inserts
    if config.scope == ExportScope::SchemaAndData || config.scope == ExportScope::DataOnly {
        if let Some(res) = data {
            if !res.rows.is_empty() {
                let inserts = export_sql_inserts_with_family(
                    res,
                    &config.table_name,
                    config.sql_options.batch_size,
                    config.family,
                );
                out.push_str(&inserts);
                out.push('\n');
            }
        }
    }

    // 5. Transaction Commit
    if config.sql_options.wrap_in_transaction {
        out.push_str("COMMIT;\n");
    }

    out
}

/// Serializes query results into standard CSV format.
pub fn export_csv(result: &QueryResult, include_headers: bool) -> String {
    let opts = CsvOptions {
        delimiter: ',',
        include_headers,
        null_representation: String::new(),
        quote_char: '"',
    };
    export_csv_with_options(result, &opts)
}

/// Serializes query results into delimited format with custom options.
pub fn export_csv_with_options(result: &QueryResult, options: &CsvOptions) -> String {
    let mut out = String::new();

    if options.include_headers && !result.columns.is_empty() {
        for (i, col) in result.columns.iter().enumerate() {
            if i > 0 {
                out.push(options.delimiter);
            }
            write_delimited_cell(&mut out, col, options.delimiter, options.quote_char);
        }
        out.push('\n');
    }

    for row in &result.rows {
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                out.push(options.delimiter);
            }
            match val {
                QueryValue::Null => {
                    out.push_str(&options.null_representation);
                }
                _ => {
                    let text = val.to_string();
                    write_delimited_cell(&mut out, &text, options.delimiter, options.quote_char);
                }
            }
        }
        out.push('\n');
    }

    out
}

fn write_delimited_cell(out: &mut String, text: &str, delimiter: char, quote_char: char) {
    let needs_quotes = text.contains(delimiter)
        || text.contains(quote_char)
        || text.contains('\n')
        || text.contains('\r');
    if needs_quotes {
        out.push(quote_char);
        for ch in text.chars() {
            if ch == quote_char {
                out.push(quote_char);
                out.push(quote_char);
            } else {
                out.push(ch);
            }
        }
        out.push(quote_char);
    } else {
        out.push_str(text);
    }
}

/// Serializes query results into JSON format (array of objects).
pub fn export_json(result: &QueryResult, pretty: bool) -> String {
    let rows_json = query_result_to_json_objects(result);
    let val = serde_json::Value::Array(rows_json);
    if pretty {
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| "[]".to_string())
    } else {
        serde_json::to_string(&val).unwrap_or_else(|_| "[]".to_string())
    }
}

/// Serializes query results into NDJSON format (one JSON object per line).
pub fn export_ndjson(result: &QueryResult) -> String {
    let rows_json = query_result_to_json_objects(result);
    let mut out = String::new();
    for obj in rows_json {
        if let Ok(line) = serde_json::to_string(&obj) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

fn query_result_to_json_objects(result: &QueryResult) -> Vec<serde_json::Value> {
    let mut rows_json = Vec::with_capacity(result.rows.len());

    for row in &result.rows {
        let mut map = serde_json::Map::with_capacity(result.columns.len());
        for (i, val) in row.iter().enumerate() {
            let key = result
                .columns
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("col_{i}"));
            let json_val = match val {
                QueryValue::Null => serde_json::Value::Null,
                QueryValue::Bool(b) => serde_json::Value::Bool(*b),
                QueryValue::Int(n) => serde_json::Value::Number((*n).into()),
                QueryValue::Float(f) => serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                QueryValue::String(s) => serde_json::Value::String(s.clone()),
                QueryValue::DateTime(dt) => serde_json::Value::String(dt.clone()),
                QueryValue::Bytes(b) => serde_json::Value::String(format!("\\x{}", hex_encode(b))),
            };
            map.insert(key, json_val);
        }
        rows_json.push(serde_json::Value::Object(map));
    }

    rows_json
}

/// Serializes query results into GitHub Flavored Markdown (GFM) table format.
pub fn export_markdown(result: &QueryResult) -> String {
    if result.columns.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    // Headers
    out.push_str("| ");
    for col in &result.columns {
        out.push_str(&col.replace('|', "\\|"));
        out.push_str(" | ");
    }
    out.push('\n');

    // Separators
    out.push_str("| ");
    for _ in &result.columns {
        out.push_str("--- | ");
    }
    out.push('\n');

    // Rows
    for row in &result.rows {
        out.push_str("| ");
        for val in row {
            let s = match val {
                QueryValue::Null => "NULL".to_string(),
                _ => val.to_string().replace('|', "\\|").replace('\n', " "),
            };
            out.push_str(&s);
            out.push_str(" | ");
        }
        out.push('\n');
    }

    out
}

/// Serializes query results into batch SQL INSERT statements (defaulting to SQLite/Postgres double quotes).
pub fn export_sql_inserts(result: &QueryResult, table_name: &str, batch_size: usize) -> String {
    export_sql_inserts_with_family(result, table_name, batch_size, DatabaseFamily::Sqlite)
}

/// Serializes query results into batch SQL INSERT statements with dialect quoting.
pub fn export_sql_inserts_with_family(
    result: &QueryResult,
    table_name: &str,
    batch_size: usize,
    family: DatabaseFamily,
) -> String {
    if result.columns.is_empty() || result.rows.is_empty() {
        return String::new();
    }

    let batch = if batch_size == 0 { 100 } else { batch_size };
    let mut out = String::new();

    let quoted_table = quote_ident(table_name, family);
    let col_names = result
        .columns
        .iter()
        .map(|c| quote_ident(c, family))
        .collect::<Vec<_>>()
        .join(", ");

    for chunk in result.rows.chunks(batch) {
        out.push_str(&format!(
            "INSERT INTO {quoted_table} ({col_names}) VALUES\n"
        ));

        for (row_idx, row) in chunk.iter().enumerate() {
            out.push_str("  (");
            for (col_idx, val) in row.iter().enumerate() {
                if col_idx > 0 {
                    out.push_str(", ");
                }
                match val {
                    QueryValue::Null => out.push_str("NULL"),
                    QueryValue::Bool(b) => out.push_str(if *b { "TRUE" } else { "FALSE" }),
                    QueryValue::Int(n) => out.push_str(&n.to_string()),
                    QueryValue::Float(f) => out.push_str(&f.to_string()),
                    QueryValue::String(s) => {
                        out.push('\'');
                        out.push_str(&s.replace('\'', "''"));
                        out.push('\'');
                    }
                    QueryValue::DateTime(dt) => {
                        out.push('\'');
                        out.push_str(&dt.replace('\'', "''"));
                        out.push('\'');
                    }
                    QueryValue::Bytes(b) => match family {
                        DatabaseFamily::Postgres => {
                            out.push_str(&format!("'\\x{}'::bytea", hex_encode(b)));
                        }
                        _ => {
                            out.push_str(&format!("X'{}'", hex_encode(b)));
                        }
                    },
                }
            }
            out.push(')');
            if row_idx + 1 < chunk.len() {
                out.push_str(",\n");
            } else {
                out.push_str(";\n");
            }
        }
    }

    out
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Generates a suggested export filename.
pub fn suggested_file_name(base: &str, format: ExportFormat) -> String {
    let now = chrono::Local::now();
    let timestamp = now.format("%Y%m%d_%H%M%S");
    let ext = format.extension();
    format!("{base}_{timestamp}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_query_result() -> QueryResult {
        QueryResult {
            columns: vec!["id".to_string(), "name".to_string(), "active".to_string()],
            column_types: vec![
                "INTEGER".to_string(),
                "TEXT".to_string(),
                "BOOLEAN".to_string(),
            ],
            rows: vec![
                vec![
                    QueryValue::Int(1),
                    QueryValue::String("Alice, \"Engineer\"".to_string()),
                    QueryValue::Bool(true),
                ],
                vec![
                    QueryValue::Int(2),
                    QueryValue::Null,
                    QueryValue::Bool(false),
                ],
            ],
            rows_affected: Some(0),
            execution_time_ms: Some(1),
        }
    }

    #[test]
    fn test_csv_export() {
        let res = sample_query_result();
        let csv = export_csv(&res, true);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "id,name,active");
        assert_eq!(lines[1], "1,\"Alice, \"\"Engineer\"\"\",true");
        assert_eq!(lines[2], "2,,false");
    }

    #[test]
    fn test_tsv_export() {
        let res = sample_query_result();
        let mut opts = CsvOptions::default();
        opts.delimiter = '\t';
        opts.null_representation = "\\N".to_string();
        let tsv = export_csv_with_options(&res, &opts);
        let lines: Vec<&str> = tsv.lines().collect();
        assert_eq!(lines[0], "id\tname\tactive");
        assert_eq!(lines[2], "2\t\\N\tfalse");
    }

    #[test]
    fn test_json_export() {
        let res = sample_query_result();
        let json = export_json(&res, false);
        assert!(json.contains("\"name\":\"Alice, \\\"Engineer\\\"\""));
        assert!(json.contains("\"active\":true"));
        assert!(json.contains("\"name\":null"));
    }

    #[test]
    fn test_ndjson_export() {
        let res = sample_query_result();
        let ndjson = export_ndjson(&res);
        let lines: Vec<&str> = ndjson.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"id\":1"));
        assert!(lines[1].contains("\"id\":2"));
    }

    #[test]
    fn test_markdown_export() {
        let res = sample_query_result();
        let md = export_markdown(&res);
        assert!(md.contains("| id | name | active |"));
        assert!(md.contains("| --- | --- | --- |"));
        assert!(md.contains("| 1 | Alice, \"Engineer\" | true |"));
        assert!(md.contains("| 2 | NULL | false |"));
    }

    #[test]
    fn test_sql_insert_export() {
        let res = sample_query_result();
        let sql = export_sql_inserts(&res, "users", 100);
        assert!(sql.contains("INSERT INTO \"users\" (\"id\", \"name\", \"active\") VALUES"));
        assert!(sql.contains("(1, 'Alice, \"Engineer\"', TRUE)"));
    }

    #[test]
    fn test_table_dump_full() {
        let res = sample_query_result();
        let ddl = "CREATE TABLE \"users\" (\n  \"id\" INTEGER PRIMARY KEY,\n  \"name\" TEXT,\n  \"active\" BOOLEAN\n);";
        let mut config = TableDumpConfig::default();
        config.table_name = "users".to_string();
        config.scope = ExportScope::SchemaAndData;
        config.sql_options.drop_table_if_exists = true;
        config.sql_options.wrap_in_transaction = true;

        let dump = generate_table_dump(Some(ddl), Some(&res), &config);
        assert!(dump.contains("DROP TABLE IF EXISTS \"users\";"));
        assert!(dump.contains("CREATE TABLE \"users\""));
        assert!(dump.contains("BEGIN TRANSACTION;"));
        assert!(dump.contains("INSERT INTO \"users\""));
        assert!(dump.contains("COMMIT;"));
    }

    #[test]
    fn test_table_dump_schema_only() {
        let res = sample_query_result();
        let ddl = "CREATE TABLE `users` (`id` INT);";
        let mut config = TableDumpConfig::default();
        config.table_name = "users".to_string();
        config.family = DatabaseFamily::MySql;
        config.scope = ExportScope::SchemaOnly;
        config.sql_options.wrap_in_transaction = false;

        let dump = generate_table_dump(Some(ddl), Some(&res), &config);
        assert!(dump.contains("DROP TABLE IF EXISTS `users`;"));
        assert!(dump.contains("CREATE TABLE `users` (`id` INT);"));
        assert!(!dump.contains("INSERT INTO"));
        assert!(!dump.contains("BEGIN"));
    }

    #[test]
    fn test_export_preview_truncation() {
        let mut res = sample_query_result();
        for i in 3..=25 {
            res.rows.push(vec![
                QueryValue::Int(i),
                QueryValue::String(format!("User {i}")),
                QueryValue::Bool(true),
            ]);
        }
        let config = TableDumpConfig {
            table_name: "users".to_string(),
            format: ExportFormat::Csv,
            ..Default::default()
        };
        let preview = generate_export_preview(None, Some(&res), &config, 5);
        assert!(preview.contains("Preview truncated: 20 more row(s) omitted in preview"));
    }
}
