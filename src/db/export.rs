//! Data export serializers for converting query results into CSV, JSON, Markdown, and SQL statements.

use crate::db::types::{QueryResult, QueryValue};
use serde::{Deserialize, Serialize};

/// Supported export target formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
    Markdown,
    SqlInsert,
}

impl ExportFormat {
    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::SqlInsert => "SQL Inserts",
        }
    }

    /// File extension suitable for saving.
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Markdown => "md",
            Self::SqlInsert => "sql",
        }
    }
}

/// Options controlling the serialization formatting.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub table_name: Option<String>,
    pub include_headers: bool,
    pub pretty_json: bool,
    pub batch_size: usize,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            table_name: None,
            include_headers: true,
            pretty_json: true,
            batch_size: 100,
        }
    }
}

/// Serialize a QueryResult according to the provided export options.
pub fn export_result(result: &QueryResult, options: &ExportOptions) -> String {
    match options.format {
        ExportFormat::Csv => export_csv(result, options.include_headers),
        ExportFormat::Json => export_json(result, options.pretty_json),
        ExportFormat::Markdown => export_markdown(result),
        ExportFormat::SqlInsert => {
            let tbl = options.table_name.as_deref().unwrap_or("exported_data");
            export_sql_inserts(result, tbl, options.batch_size)
        }
    }
}

/// Serializes query results into RFC 4180 compliant CSV format.
pub fn export_csv(result: &QueryResult, include_headers: bool) -> String {
    let mut out = String::new();

    if include_headers && !result.columns.is_empty() {
        for (i, col) in result.columns.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write_csv_cell(&mut out, col);
        }
        out.push('\n');
    }

    for row in &result.rows {
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            match val {
                QueryValue::Null => {} // Empty field for NULL in CSV
                QueryValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                QueryValue::Int(n) => out.push_str(&n.to_string()),
                QueryValue::Float(f) => out.push_str(&f.to_string()),
                QueryValue::String(s) => write_csv_cell(&mut out, s),
                QueryValue::DateTime(dt) => write_csv_cell(&mut out, dt),
                QueryValue::Bytes(b) => {
                    let hex_str = format!("\\x{}", hex_encode(b));
                    write_csv_cell(&mut out, &hex_str);
                }
            }
        }
        out.push('\n');
    }

    out
}

fn write_csv_cell(out: &mut String, text: &str) {
    let needs_quotes = text.contains(',') || text.contains('"') || text.contains('\n') || text.contains('\r');
    if needs_quotes {
        out.push('"');
        for ch in text.chars() {
            if ch == '"' {
                out.push('"');
                out.push('"');
            } else {
                out.push(ch);
            }
        }
        out.push('"');
    } else {
        out.push_str(text);
    }
}

/// Serializes query results into JSON format (array of objects).
pub fn export_json(result: &QueryResult, pretty: bool) -> String {
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

    let val = serde_json::Value::Array(rows_json);
    if pretty {
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| "[]".to_string())
    } else {
        serde_json::to_string(&val).unwrap_or_else(|_| "[]".to_string())
    }
}

/// Serializes query results into GitHub Flavored Markdown (GFM) table format.
pub fn export_markdown(result: &QueryResult) -> String {
    if result.columns.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    // Headers
    out.push_str("| ");
    for (i, col) in result.columns.iter().enumerate() {
        if i > 0 {
            out.push_str(" | ");
        }
        out.push_str(&escape_markdown_cell(col));
    }
    out.push_str(" |\n");

    // Separators
    out.push_str("| ");
    for (i, _) in result.columns.iter().enumerate() {
        if i > 0 {
            out.push_str(" | ");
        }
        out.push_str("---");
    }
    out.push_str(" |\n");

    // Rows
    for row in &result.rows {
        out.push_str("| ");
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                out.push_str(" | ");
            }
            let text = match val {
                QueryValue::Null => "NULL".to_string(),
                QueryValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                QueryValue::Int(n) => n.to_string(),
                QueryValue::Float(f) => format!("{f:.4}"),
                QueryValue::String(s) => s.clone(),
                QueryValue::DateTime(dt) => dt.clone(),
                QueryValue::Bytes(b) => format!("<blob: {} bytes>", b.len()),
            };
            out.push_str(&escape_markdown_cell(&text));
        }
        out.push_str(" |\n");
    }

    out
}

fn escape_markdown_cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ").replace('\r', "")
}

/// Serializes query results into batched SQL INSERT statements.
pub fn export_sql_inserts(result: &QueryResult, table_name: &str, batch_size: usize) -> String {
    if result.columns.is_empty() || result.rows.is_empty() {
        return format!("-- No rows to export for table '{table_name}'\n");
    }

    let batch = if batch_size == 0 { 100 } else { batch_size };
    let mut out = String::new();

    let columns_part = result
        .columns
        .iter()
        .map(|c| format!("\"{}\"", c.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(", ");

    for chunk in result.rows.chunks(batch) {
        out.push_str(&format!("INSERT INTO \"{}\" ({}) VALUES\n", table_name, columns_part));
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
                    QueryValue::Bytes(b) => {
                        out.push_str(&format!("X'{}'", hex_encode(b)));
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_query_result() -> QueryResult {
        QueryResult {
            columns: vec!["id".to_string(), "name".to_string(), "active".to_string()],
            column_types: vec!["INTEGER".to_string(), "TEXT".to_string(), "BOOLEAN".to_string()],
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
            execution_time_ms: Some(12),
            rows_affected: None,
        }
    }

    #[test]
    fn test_csv_export() {
        let res = sample_query_result();
        let csv = export_csv(&res, true);
        assert!(csv.contains("id,name,active\n"));
        assert!(csv.contains("1,\"Alice, \"\"Engineer\"\"\",true\n"));
        assert!(csv.contains("2,,false\n"));
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
        assert!(sql.contains("(2, NULL, FALSE);"));
    }
}
