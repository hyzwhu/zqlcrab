//! SQL statement generator for atomic tabular updates and deletions across database dialects.

use crate::db::changeset::GridChangeset;
use crate::db::types::{ColumnInfo, DatabaseFamily, QueryValue, TableInfo, quote_ident};
use std::collections::BTreeMap;

/// Detailed review plan generated from pending changeset modifications.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlReviewPlan {
    pub table_name: String,
    pub schema_name: Option<String>,
    pub database_family: DatabaseFamily,
    pub inserts_count: usize,
    pub updates_count: usize,
    pub deletes_count: usize,
    pub has_primary_key: bool,
    pub primary_keys: Vec<String>,
    pub warnings: Vec<String>,
    pub statements: Vec<String>,
    pub full_script: String,
}

/// Generates a structured SQL review plan and transaction script for pending changeset edits.
pub fn generate_review_plan(
    table_name: &str,
    schema_name: Option<&str>,
    family: DatabaseFamily,
    columns: &[ColumnInfo],
    grid_columns: &[String],
    original_rows: &[Vec<QueryValue>],
    changeset: &GridChangeset,
) -> SqlReviewPlan {
    let mut warnings = Vec::new();
    let mut statements = Vec::new();

    // 1. Identify primary keys from schema metadata
    let pk_names: Vec<String> = columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| c.name.clone())
        .collect();

    let has_primary_key = !pk_names.is_empty();

    if !has_primary_key && changeset.is_dirty() {
        warnings.push(format!(
            "Table '{table_name}' has no primary key. WHERE conditions will match all original column values, which may affect duplicate rows if any exist."
        ));
    }

    // Qualified table identifier (e.g. "public"."users" or `mydb`.`users`)
    let qualified_table = match schema_name {
        Some(s) if !s.trim().is_empty() => {
            format!(
                "{}.{}",
                quote_ident(s, family),
                quote_ident(table_name, family)
            )
        }
        _ => quote_ident(table_name, family),
    };

    // 2. Generate INSERT statements for uncommitted new rows
    let mut inserts_count = 0;
    for insertion in &changeset.inserted_rows {
        let mut insert_cols = Vec::new();
        let mut insert_vals = Vec::new();

        for (col_idx, col_name) in grid_columns.iter().enumerate() {
            let val = insertion.values.get(col_idx).unwrap_or(&QueryValue::Null);
            let col_meta = columns
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(col_name));

            // Determine whether to omit an auto-increment or serial column when value is Null or <auto>
            let is_sqlite = family == DatabaseFamily::Sqlite;
            let is_auto = col_meta
                .map(|c| {
                    c.is_auto_increment
                        || c.data_type.to_lowercase().contains("serial")
                        || (is_sqlite
                            && c.is_primary_key
                            && c.data_type.to_lowercase().contains("int"))
                })
                .unwrap_or(false);

            let is_auto_placeholder = match val {
                QueryValue::Null => is_auto,
                QueryValue::String(s) if s.trim().eq_ignore_ascii_case("<auto>") => true,
                _ => false,
            };

            let has_default_and_not_nullable = col_meta
                .map(|c| !c.is_nullable && (c.default_value.is_some() || c.is_auto_increment))
                .unwrap_or(false);

            let is_default_placeholder = match val {
                QueryValue::Null => has_default_and_not_nullable,
                QueryValue::String(s) if s.trim().eq_ignore_ascii_case("<default>") => true,
                _ => false,
            };

            if is_auto_placeholder || is_default_placeholder {
                continue;
            }

            if let Some(c) = col_meta {
                if !c.is_nullable
                    && !is_auto
                    && c.default_value.is_none()
                    && matches!(val, QueryValue::Null)
                {
                    warnings.push(format!(
                        "Column '{}' in new row #{} is NOT NULL and has no default value, but is currently NULL.",
                        col_name, inserts_count + 1
                    ));
                }
            }

            insert_cols.push(quote_ident(col_name, family));
            insert_vals.push(format_query_value(val, family));
        }

        if insert_cols.is_empty() {
            match family {
                DatabaseFamily::MySql => {
                    statements.push(format!("INSERT INTO {qualified_table} () VALUES ();"));
                }
                _ => {
                    statements.push(format!("INSERT INTO {qualified_table} DEFAULT VALUES;"));
                }
            }
        } else {
            let cols_str = insert_cols.join(", ");
            let vals_str = insert_vals.join(", ");
            // Multi-line formatting if wide or many columns to avoid modal viewport horizontal sprawl
            if cols_str.len() + vals_str.len() > 80 || insert_cols.len() > 5 {
                let indented_cols = insert_cols
                    .iter()
                    .map(|c| format!("    {c}"))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let indented_vals = insert_vals
                    .iter()
                    .map(|v| format!("    {v}"))
                    .collect::<Vec<_>>()
                    .join(",\n");
                statements.push(format!(
                    "INSERT INTO {qualified_table} (\n{indented_cols}\n) VALUES (\n{indented_vals}\n);"
                ));
            } else {
                statements.push(format!(
                    "INSERT INTO {qualified_table} ({cols_str}) VALUES ({vals_str});"
                ));
            }
        }
        inserts_count += 1;
    }

    // 3. Group cell updates by row index (skip rows that are also marked deleted)
    let mut row_updates: BTreeMap<usize, Vec<(usize, String, QueryValue)>> = BTreeMap::new();
    for (&(r_idx, c_idx), edit) in &changeset.cell_updates {
        if changeset.is_row_deleted(r_idx) {
            continue; // Deletion takes precedence
        }
        row_updates.entry(r_idx).or_default().push((
            c_idx,
            edit.column_name.clone(),
            edit.new_value.clone(),
        ));
    }

    let mut updates_count = 0;

    // 3. Generate UPDATE statements
    for (r_idx, edits) in row_updates {
        let Some(orig_row) = original_rows.get(r_idx) else {
            continue;
        };

        // SET clause
        let mut set_clauses = Vec::new();
        for (_, col_name, new_val) in edits {
            let col_quoted = quote_ident(&col_name, family);
            let val_str = format_query_value(&new_val, family);
            set_clauses.push(format!("{col_quoted} = {val_str}"));
        }

        // WHERE clause
        let where_clause =
            build_row_where_clause(family, columns, grid_columns, orig_row, &pk_names);

        let set_str = set_clauses.join(", ");
        statements.push(format!(
            "UPDATE {qualified_table}\nSET {set_str}\nWHERE {where_clause};"
        ));
        updates_count += 1;
    }

    // 4. Generate DELETE statements
    let mut deletes_count = 0;
    // Sort deletion keys for deterministic output
    let mut del_indices: Vec<usize> = changeset.deleted_rows.keys().copied().collect();
    del_indices.sort_unstable();

    for r_idx in del_indices {
        let del_info = &changeset.deleted_rows[&r_idx];
        let where_clause = build_row_where_clause(
            family,
            columns,
            grid_columns,
            &del_info.original_row,
            &pk_names,
        );

        statements.push(format!(
            "DELETE FROM {qualified_table}\nWHERE {where_clause};"
        ));
        deletes_count += 1;
    }

    // 6. Wrap inside an atomic transaction script
    let full_script = build_transaction_script(family, &statements);

    SqlReviewPlan {
        table_name: table_name.to_string(),
        schema_name: schema_name.map(|s| s.to_string()),
        database_family: family,
        inserts_count,
        updates_count,
        deletes_count,
        has_primary_key,
        primary_keys: pk_names,
        warnings,
        statements,
        full_script,
    }
}

/// Constructs the WHERE clause identifying a specific row based on Primary Key or full row fallback.
fn build_row_where_clause(
    family: DatabaseFamily,
    columns: &[ColumnInfo],
    grid_columns: &[String],
    row_values: &[QueryValue],
    pk_names: &[String],
) -> String {
    let mut clauses = Vec::new();

    if !pk_names.is_empty() {
        // Match only primary key columns
        for pk in pk_names {
            // Find column index
            let col_idx = grid_columns
                .iter()
                .position(|c| c.eq_ignore_ascii_case(pk))
                .or_else(|| columns.iter().position(|c| c.name.eq_ignore_ascii_case(pk)));

            if let Some(idx) = col_idx {
                if let Some(val) = row_values.get(idx) {
                    let col_quoted = quote_ident(pk, family);
                    if val.is_null() {
                        clauses.push(format!("{col_quoted} IS NULL"));
                    } else {
                        clauses.push(format!(
                            "{col_quoted} = {}",
                            format_query_value(val, family)
                        ));
                    }
                }
            }
        }
    }

    // If no PK was found or couldn't match PK values, match all non-blob original column values
    if clauses.is_empty() {
        for (idx, col_name) in grid_columns.iter().enumerate() {
            if let Some(val) = row_values.get(idx) {
                // Skip blobs in WHERE matching fallback
                if matches!(val, QueryValue::Bytes(_)) {
                    continue;
                }
                let col_quoted = quote_ident(col_name, family);
                if val.is_null() {
                    clauses.push(format!("{col_quoted} IS NULL"));
                } else {
                    clauses.push(format!(
                        "{col_quoted} = {}",
                        format_query_value(val, family)
                    ));
                }
            }
        }
    }

    if clauses.is_empty() {
        "1 = 1".to_string()
    } else {
        clauses.join(" AND ")
    }
}

/// Formats a QueryValue as a dialect-safe SQL literal.
pub fn format_query_value(val: &QueryValue, family: DatabaseFamily) -> String {
    match val {
        QueryValue::Null => "NULL".to_string(),
        QueryValue::Bool(b) => match family {
            DatabaseFamily::Postgres => if *b { "TRUE" } else { "FALSE" }.to_string(),
            DatabaseFamily::MySql | DatabaseFamily::Sqlite => {
                if *b { "1" } else { "0" }.to_string()
            }
        },
        QueryValue::Int(i) => i.to_string(),
        QueryValue::Float(f) => {
            let s = format!("{f}");
            if s.contains('.') { s } else { format!("{f}.0") }
        }
        QueryValue::String(s) => match family {
            DatabaseFamily::MySql => {
                let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
                format!("'{escaped}'")
            }
            DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
                let escaped = s.replace('\'', "''");
                format!("'{escaped}'")
            }
        },
        QueryValue::DateTime(dt) => {
            let escaped = dt.replace('\'', "''");
            format!("'{escaped}'")
        }
        QueryValue::Bytes(b) => match family {
            DatabaseFamily::Postgres => format!("'\\x{}'::bytea", hex_encode(b)),
            DatabaseFamily::MySql | DatabaseFamily::Sqlite => format!("X'{}'", hex_encode(b)),
        },
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02X}", b);
    }
    s
}

/// Encloses statements within an atomic transaction.
fn build_transaction_script(family: DatabaseFamily, statements: &[String]) -> String {
    if statements.is_empty() {
        return "-- No pending changes to execute".to_string();
    }

    let joined_stmts = statements.join("\n\n");

    match family {
        DatabaseFamily::MySql => {
            format!("START TRANSACTION;\n\n{joined_stmts}\n\nCOMMIT;")
        }
        DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
            format!("BEGIN;\n\n{joined_stmts}\n\nCOMMIT;")
        }
    }
}

/// Extracts the target table name (and optional schema name) from a SQL query string.
/// Supports SELECT ... FROM, INSERT INTO, UPDATE, and DELETE FROM queries across dialects.
pub fn extract_table_from_sql(sql: &str) -> Option<(Option<String>, String)> {
    let cleaned = crate::db::safety::QuerySafetyValidator::clean_sql(sql);
    if cleaned.is_empty() {
        return None;
    }

    let words: Vec<&str> = cleaned.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    let mut target_token: Option<&str> = None;

    for (i, &word) in words.iter().enumerate() {
        let upper = word.to_ascii_uppercase();
        let stripped_upper = upper.trim_matches(|c: char| !c.is_alphanumeric());

        if stripped_upper == "FROM" || stripped_upper == "INTO" || stripped_upper == "UPDATE" {
            let mut next_idx = i + 1;
            // Skip optional keywords like ONLY (Postgres: SELECT * FROM ONLY users)
            if next_idx < words.len() && words[next_idx].eq_ignore_ascii_case("ONLY") {
                next_idx += 1;
            }
            if next_idx < words.len() {
                target_token = Some(words[next_idx]);
                break;
            }
        }
    }

    let raw_target = target_token?;
    let trimmed = raw_target
        .trim_matches(|c| c == ';' || c == ',' || c == ')' || c == '(')
        .trim();
    if trimmed.is_empty() || trimmed.starts_with('(') {
        return None;
    }

    parse_qualified_identifier(trimmed)
}

fn parse_qualified_identifier(ident: &str) -> Option<(Option<String>, String)> {
    let clean_ident = ident.trim();
    if clean_ident.is_empty() {
        return None;
    }

    // Split on '.' while handling quotes around parts
    let parts: Vec<&str> = clean_ident.split('.').collect();
    if parts.len() == 1 {
        let tbl = strip_identifier_quotes(parts[0]);
        if tbl.is_empty() {
            None
        } else {
            Some((None, tbl))
        }
    } else if parts.len() == 2 {
        let schema = strip_identifier_quotes(parts[0]);
        let tbl = strip_identifier_quotes(parts[1]);
        if tbl.is_empty() {
            None
        } else {
            Some((
                if schema.is_empty() {
                    None
                } else {
                    Some(schema)
                },
                tbl,
            ))
        }
    } else {
        let schema = strip_identifier_quotes(parts[parts.len() - 2]);
        let tbl = strip_identifier_quotes(parts[parts.len() - 1]);
        if tbl.is_empty() {
            None
        } else {
            Some((
                if schema.is_empty() {
                    None
                } else {
                    Some(schema)
                },
                tbl,
            ))
        }
    }
}

fn strip_identifier_quotes(s: &str) -> String {
    s.trim()
        .trim_matches(|c| c == '`' || c == '"' || c == '\'' || c == '[' || c == ']')
        .to_string()
}

/// Splits a comma-separated SQL column list respecting quotes (`"`, `` ` ``, `'`, `[`..`]`).
/// Useful for parsing index column specifications where quoted identifiers or expressions may contain commas.
pub fn parse_sql_column_list(s: &str) -> Vec<String> {
    let mut cols = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    let mut in_bracket = false;

    for ch in s.chars() {
        match ch {
            '"' | '`' | '\'' => {
                if let Some(q) = in_quote {
                    if q == ch {
                        in_quote = None;
                    }
                } else if !in_bracket {
                    in_quote = Some(ch);
                }
                current.push(ch);
            }
            '[' if in_quote.is_none() => {
                in_bracket = true;
                current.push(ch);
            }
            ']' if in_bracket && in_quote.is_none() => {
                in_bracket = false;
                current.push(ch);
            }
            ',' if in_quote.is_none() && !in_bracket => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    cols.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        cols.push(trimmed.to_string());
    }

    cols
}

/// Extracts the clean, unquoted base column identifier from an index column item,
/// stripping enclosing identifier quotes and optional trailing order qualifiers (ASC / DESC).
pub fn extract_base_column_name(item: &str) -> String {
    let trimmed = item.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Check if ends with ASC or DESC
    let mut clean_item = trimmed;
    let upper = clean_item.to_ascii_uppercase();
    if upper.ends_with(" ASC") {
        clean_item = clean_item[..clean_item.len() - 4].trim();
    } else if upper.ends_with(" DESC") {
        clean_item = clean_item[..clean_item.len() - 5].trim();
    }

    strip_identifier_quotes(clean_item)
}

/// Determines whether an index column entry matches a given column name.
/// Accurately differentiates quoted identifiers (case-sensitive) and unquoted identifiers (case-insensitive).
pub fn column_matches_index_spec(item: &str, col_name: &str) -> bool {
    let base = extract_base_column_name(item);
    let trimmed = item.trim();
    let is_quoted =
        trimmed.starts_with('"') || trimmed.starts_with('`') || trimmed.starts_with('[');

    if is_quoted {
        base == col_name
    } else {
        base.eq_ignore_ascii_case(col_name)
    }
}

/// Splits a SQL script into individual executable statements, properly handling:
/// - Single quotes `'...'` and escaped single quotes `''`
/// - Double quotes `"..."` and escaped double quotes `""`
/// - MySQL backticks `` `...` ``
/// - PostgreSQL dollar quotes `$$...$$` or `$tag$...$tag$`
/// - Line comments `-- ...\n`
/// - Block comments `/* ... */` (including nested block comments)
pub fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;

    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        Backtick,
        DollarQuote(String),
        LineComment,
        BlockComment(usize),
    }

    let mut state = State::Normal;

    while i < len {
        let ch = chars[i];
        let next_ch = if i + 1 < len { Some(chars[i + 1]) } else { None };

        match &mut state {
            State::Normal => {
                if ch == '-' && next_ch == Some('-') {
                    state = State::LineComment;
                    current.push(ch);
                    current.push('-');
                    i += 2;
                } else if ch == '/' && next_ch == Some('*') {
                    state = State::BlockComment(1);
                    current.push(ch);
                    current.push('*');
                    i += 2;
                } else if ch == '\'' {
                    state = State::SingleQuote;
                    current.push(ch);
                    i += 1;
                } else if ch == '"' {
                    state = State::DoubleQuote;
                    current.push(ch);
                    i += 1;
                } else if ch == '`' {
                    state = State::Backtick;
                    current.push(ch);
                    i += 1;
                } else if ch == '$' {
                    // Check for dollar-quoted tag in PostgreSQL: $[a-zA-Z0-9_]*$
                    let mut tag_end = None;
                    for j in (i + 1)..len {
                        let c = chars[j];
                        if c == '$' {
                            tag_end = Some(j);
                            break;
                        } else if !c.is_alphanumeric() && c != '_' {
                            break;
                        }
                    }
                    if let Some(end_idx) = tag_end {
                        let tag: String = chars[i..=end_idx].iter().collect();
                        current.push_str(&tag);
                        i = end_idx + 1;
                        state = State::DollarQuote(tag);
                    } else {
                        current.push(ch);
                        i += 1;
                    }
                } else if ch == ';' {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        statements.push(trimmed.to_string());
                    }
                    current.clear();
                    i += 1;
                } else {
                    current.push(ch);
                    i += 1;
                }
            }
            State::LineComment => {
                current.push(ch);
                i += 1;
                if ch == '\n' {
                    state = State::Normal;
                }
            }
            State::BlockComment(depth) => {
                if ch == '/' && next_ch == Some('*') {
                    *depth += 1;
                    current.push('/');
                    current.push('*');
                    i += 2;
                } else if ch == '*' && next_ch == Some('/') {
                    *depth -= 1;
                    current.push('*');
                    current.push('/');
                    i += 2;
                    if *depth == 0 {
                        state = State::Normal;
                    }
                } else {
                    current.push(ch);
                    i += 1;
                }
            }
            State::SingleQuote => {
                current.push(ch);
                i += 1;
                if ch == '\'' {
                    if i < len && chars[i] == '\'' {
                        // Escaped ''
                        current.push('\'');
                        i += 1;
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::DoubleQuote => {
                current.push(ch);
                i += 1;
                if ch == '"' {
                    if i < len && chars[i] == '"' {
                        // Escaped ""
                        current.push('"');
                        i += 1;
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::Backtick => {
                current.push(ch);
                i += 1;
                if ch == '`' {
                    if i < len && chars[i] == '`' {
                        // Escaped ``
                        current.push('`');
                        i += 1;
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::DollarQuote(tag) => {
                let tag_len = tag.chars().count();
                if i + tag_len <= len {
                    let slice: String = chars[i..i + tag_len].iter().collect();
                    if slice == *tag {
                        current.push_str(&slice);
                        i += tag_len;
                        state = State::Normal;
                        continue;
                    }
                }
                current.push(ch);
                i += 1;
            }
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}

/// Safely truncates a SQL statement into a single-line preview snippet of at most `max_chars` characters,
/// respecting UTF-8 character boundaries.
pub fn truncate_sql_snippet(stmt: &str, max_chars: usize) -> String {
    let single_line = stmt.trim().replace('\n', " ").replace('\r', " ");
    let mut chars = single_line.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

/// Returns standard data types for each database dialect
pub fn dialect_data_types(family: DatabaseFamily) -> &'static [&'static str] {
    match family {
        DatabaseFamily::Postgres => &[
            "SERIAL",
            "BIGSERIAL",
            "INTEGER",
            "BIGINT",
            "SMALLINT",
            "VARCHAR(255)",
            "TEXT",
            "BOOLEAN",
            "TIMESTAMPTZ",
            "TIMESTAMP",
            "DATE",
            "TIME",
            "NUMERIC(10,2)",
            "DOUBLE PRECISION",
            "REAL",
            "JSONB",
            "UUID",
            "BYTEA",
        ],
        DatabaseFamily::MySql => &[
            "INT",
            "BIGINT",
            "TINYINT",
            "SMALLINT",
            "VARCHAR(255)",
            "TEXT",
            "LONGTEXT",
            "DATETIME",
            "TIMESTAMP",
            "DATE",
            "TIME",
            "DECIMAL(10,2)",
            "DOUBLE",
            "FLOAT",
            "JSON",
            "BOOLEAN",
            "BLOB",
        ],
        DatabaseFamily::Sqlite => &[
            "INTEGER",
            "TEXT",
            "REAL",
            "BLOB",
            "NUMERIC",
            "BOOLEAN",
        ],
    }
}

/// Returns quick presets for the Create Table modal header
pub fn dialect_presets(family: DatabaseFamily) -> &'static [&'static str] {
    match family {
        DatabaseFamily::Postgres => &[
            "SERIAL",
            "BIGINT",
            "VARCHAR(255)",
            "TIMESTAMPTZ",
            "TEXT",
            "JSONB",
        ],
        DatabaseFamily::MySql => &[
            "INT",
            "BIGINT",
            "VARCHAR(255)",
            "DATETIME",
            "TEXT",
            "JSON",
        ],
        DatabaseFamily::Sqlite => &[
            "INTEGER",
            "TEXT",
            "REAL",
            "BLOB",
        ],
    }
}

/// Formats an individual index column expression for DDL generation.
/// If the expression includes order qualifiers (e.g. `col DESC`), the identifier part is quoted while preserving order.
/// If already quoted, it is preserved. Otherwise, it is safely quoted with `quote_ident`.
pub fn format_index_column_expr(expr: &str, family: DatabaseFamily) -> String {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let upper = trimmed.to_ascii_uppercase();
    let (ident_part, order_part) = if upper.ends_with(" ASC") {
        (trimmed[..trimmed.len() - 4].trim(), " ASC")
    } else if upper.ends_with(" DESC") {
        (trimmed[..trimmed.len() - 5].trim(), " DESC")
    } else {
        (trimmed, "")
    };

    let already_quoted = (ident_part.starts_with('"') && ident_part.ends_with('"'))
        || (ident_part.starts_with('`') && ident_part.ends_with('`'))
        || (ident_part.starts_with('[') && ident_part.ends_with(']'));

    let quoted_ident = if already_quoted {
        ident_part.to_string()
    } else {
        quote_ident(ident_part, family)
    };

    format!("{quoted_ident}{order_part}")
}

/// Column definition for table creation DDL generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String,
    pub is_primary_key: bool,
    pub is_nullable: bool,
    pub is_auto_increment: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

impl ColumnDef {
    pub fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            is_primary_key: false,
            is_nullable: true,
            is_auto_increment: false,
            default_value: None,
            comment: None,
        }
    }

    pub fn primary_key(mut self, pk: bool) -> Self {
        self.is_primary_key = pk;
        if pk {
            self.is_nullable = false;
        }
        self
    }

    pub fn nullable(mut self, nullable: bool) -> Self {
        self.is_nullable = nullable;
        self
    }

    pub fn auto_increment(mut self, auto: bool) -> Self {
        self.is_auto_increment = auto;
        if auto {
            self.is_primary_key = true;
            self.is_nullable = false;
        }
        self
    }

    pub fn default_value(mut self, default: Option<String>) -> Self {
        self.default_value = default;
        self
    }

    pub fn comment(mut self, comment: Option<String>) -> Self {
        self.comment = comment;
        self
    }
}

/// Type of index (normal or unique).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableIndexType {
    #[default]
    Normal,
    Unique,
}

impl TableIndexType {
    pub fn display_name(&self) -> &'static str {
        match self {
            TableIndexType::Normal => "INDEX",
            TableIndexType::Unique => "UNIQUE",
        }
    }
}

/// Index definition for table creation DDL generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableIndexDef {
    pub name: String,
    pub index_type: TableIndexType,
    pub columns: Vec<String>,
}

impl TableIndexDef {
    pub fn new(name: impl Into<String>, columns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            index_type: TableIndexType::Normal,
            columns,
        }
    }

    pub fn unique(mut self, is_unique: bool) -> Self {
        self.index_type = if is_unique {
            TableIndexType::Unique
        } else {
            TableIndexType::Normal
        };
        self
    }
}

/// Specifications for creating a new database table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTableDef {
    pub table_name: String,
    pub schema: Option<String>,
    pub columns: Vec<ColumnDef>,
    pub indexes: Vec<TableIndexDef>,
    pub comment: Option<String>,
}

impl CreateTableDef {
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            schema: None,
            columns: Vec::new(),
            indexes: Vec::new(),
            comment: None,
        }
    }

    pub fn schema(mut self, schema: Option<String>) -> Self {
        self.schema = schema;
        self
    }

    pub fn column(mut self, column: ColumnDef) -> Self {
        self.columns.push(column);
        self
    }

    pub fn index(mut self, index: TableIndexDef) -> Self {
        self.indexes.push(index);
        self
    }

    pub fn comment(mut self, comment: Option<String>) -> Self {
        self.comment = comment;
        self
    }
}

/// Generates dialect-specific CREATE TABLE SQL DDL for SQLite, PostgreSQL, or MySQL.
pub fn generate_create_table_sql(
    def: &CreateTableDef,
    family: DatabaseFamily,
) -> Result<String, String> {
    let table_name = def.table_name.trim();
    if table_name.is_empty() {
        return Err("Table name cannot be empty".to_string());
    }

    let valid_cols: Vec<&ColumnDef> = def
        .columns
        .iter()
        .filter(|c| !c.name.trim().is_empty())
        .collect();

    if valid_cols.is_empty() {
        return Err("At least one column with a valid name is required".to_string());
    }

    // Check duplicate column names
    let mut seen = std::collections::HashSet::new();
    for col in &valid_cols {
        let name_lower = col.name.trim().to_lowercase();
        if !seen.insert(name_lower) {
            return Err(format!("Duplicate column name: '{}'", col.name.trim()));
        }
    }

    let quoted_table = if let Some(schema) = &def.schema {
        let schema_trimmed = schema.trim();
        if !schema_trimmed.is_empty() && !schema_trimmed.eq_ignore_ascii_case("main") {
            format!(
                "{}.{}",
                quote_ident(schema_trimmed, family),
                quote_ident(table_name, family)
            )
        } else {
            quote_ident(table_name, family)
        }
    } else {
        quote_ident(table_name, family)
    };

    let pks: Vec<&ColumnDef> = valid_cols
        .iter()
        .filter(|c| c.is_primary_key)
        .copied()
        .collect();

    let mut col_clauses = Vec::new();
    let mut post_statements = Vec::new();

    match family {
        DatabaseFamily::Sqlite => {
            let single_pk_auto = pks.len() == 1 && pks[0].is_auto_increment;
            let single_pk = pks.len() == 1 && !pks[0].is_auto_increment;

            for col in &valid_cols {
                let col_name = col.name.trim();
                let col_type = col.data_type.trim();
                let quoted_col = quote_ident(col_name, family);

                if single_pk_auto && col.is_primary_key {
                    // SQLite requires INTEGER PRIMARY KEY AUTOINCREMENT
                    col_clauses.push(format!(
                        "    {quoted_col} INTEGER PRIMARY KEY AUTOINCREMENT"
                    ));
                    continue;
                }

                let mut clause = format!("    {quoted_col}");
                if !col_type.is_empty() {
                    clause.push_str(&format!(" {col_type}"));
                } else {
                    clause.push_str(" TEXT");
                }

                if single_pk && col.is_primary_key {
                    clause.push_str(" PRIMARY KEY");
                } else if !col.is_nullable {
                    clause.push_str(" NOT NULL");
                }

                if let Some(ref def_val) = col.default_value {
                    let d = def_val.trim();
                    if !d.is_empty() {
                        clause.push_str(&format!(" DEFAULT {d}"));
                    }
                }

                col_clauses.push(clause);
            }

            // Composite primary key
            if pks.len() > 1 {
                let pk_cols = pks
                    .iter()
                    .map(|c| quote_ident(c.name.trim(), family))
                    .collect::<Vec<_>>()
                    .join(", ");
                col_clauses.push(format!("    PRIMARY KEY ({pk_cols})"));
            }

            let mut stmts = vec![format!(
                "CREATE TABLE {quoted_table} (\n{}\n);",
                col_clauses.join(",\n")
            )];

            for idx in &def.indexes {
                let valid_idx_cols: Vec<&str> = idx
                    .columns
                    .iter()
                    .map(|c| c.trim())
                    .filter(|c| !c.is_empty())
                    .collect();
                if valid_idx_cols.is_empty() {
                    continue;
                }
                let idx_name = idx.name.trim();
                let actual_name = if idx_name.is_empty() {
                    let sanitized: Vec<String> = valid_idx_cols
                        .iter()
                        .map(|c| extract_base_column_name(c))
                        .collect();
                    format!("idx_{}_{}", table_name, sanitized.join("_"))
                } else {
                    idx_name.to_string()
                };
                let quoted_idx = quote_ident(&actual_name, family);
                let cols_str = valid_idx_cols
                    .iter()
                    .map(|c| format_index_column_expr(c, family))
                    .collect::<Vec<_>>()
                    .join(", ");
                let unique_str = match idx.index_type {
                    TableIndexType::Unique => "UNIQUE ",
                    TableIndexType::Normal => "",
                };
                stmts.push(format!(
                    "CREATE {unique_str}INDEX {quoted_idx} ON {quoted_table} ({cols_str});"
                ));
            }

            Ok(stmts.join("\n\n"))
        }

        DatabaseFamily::Postgres => {
            let single_pk = pks.len() == 1;

            for col in &valid_cols {
                let col_name = col.name.trim();
                let mut col_type = col.data_type.trim().to_string();
                let quoted_col = quote_ident(col_name, family);

                if col.is_auto_increment {
                    if col_type.to_lowercase().contains("big") {
                        col_type = "BIGSERIAL".to_string();
                    } else {
                        col_type = "SERIAL".to_string();
                    }
                } else if col_type.is_empty() {
                    col_type = "TEXT".to_string();
                }

                let mut clause = format!("    {quoted_col} {col_type}");

                if single_pk && col.is_primary_key {
                    clause.push_str(" PRIMARY KEY");
                } else if !col.is_nullable {
                    clause.push_str(" NOT NULL");
                }

                if let Some(ref def_val) = col.default_value {
                    let d = def_val.trim();
                    if !d.is_empty() {
                        clause.push_str(&format!(" DEFAULT {d}"));
                    }
                }

                col_clauses.push(clause);

                if let Some(ref comment) = col.comment {
                    let c = comment.trim();
                    if !c.is_empty() {
                        post_statements.push(format!(
                            "COMMENT ON COLUMN {quoted_table}.{quoted_col} IS '{}';",
                            c.replace('\'', "''")
                        ));
                    }
                }
            }

            if pks.len() > 1 {
                let pk_cols = pks
                    .iter()
                    .map(|c| quote_ident(c.name.trim(), family))
                    .collect::<Vec<_>>()
                    .join(", ");
                col_clauses.push(format!("    PRIMARY KEY ({pk_cols})"));
            }

            if let Some(ref t_comment) = def.comment {
                let c = t_comment.trim();
                if !c.is_empty() {
                    post_statements.push(format!(
                        "COMMENT ON TABLE {quoted_table} IS '{}';",
                        c.replace('\'', "''")
                    ));
                }
            }

            for idx in &def.indexes {
                let valid_idx_cols: Vec<&str> = idx
                    .columns
                    .iter()
                    .map(|c| c.trim())
                    .filter(|c| !c.is_empty())
                    .collect();
                if valid_idx_cols.is_empty() {
                    continue;
                }
                let idx_name = idx.name.trim();
                let actual_name = if idx_name.is_empty() {
                    let sanitized: Vec<String> = valid_idx_cols
                        .iter()
                        .map(|c| extract_base_column_name(c))
                        .collect();
                    format!("idx_{}_{}", table_name, sanitized.join("_"))
                } else {
                    idx_name.to_string()
                };
                let quoted_idx = quote_ident(&actual_name, family);
                let cols_str = valid_idx_cols
                    .iter()
                    .map(|c| format_index_column_expr(c, family))
                    .collect::<Vec<_>>()
                    .join(", ");
                let unique_str = match idx.index_type {
                    TableIndexType::Unique => "UNIQUE ",
                    TableIndexType::Normal => "",
                };
                post_statements.push(format!(
                    "CREATE {unique_str}INDEX {quoted_idx} ON {quoted_table} ({cols_str});"
                ));
            }

            let mut stmts = vec![format!(
                "CREATE TABLE {quoted_table} (\n{}\n);",
                col_clauses.join(",\n")
            )];
            if !post_statements.is_empty() {
                stmts.extend(post_statements);
            }
            Ok(stmts.join("\n\n"))
        }

        DatabaseFamily::MySql => {
            for col in &valid_cols {
                let col_name = col.name.trim();
                let mut col_type = col.data_type.trim().to_string();
                let quoted_col = quote_ident(col_name, family);

                if col_type.is_empty() {
                    col_type = "VARCHAR(255)".to_string();
                }

                let mut clause = format!("    {quoted_col} {col_type}");

                if col.is_auto_increment {
                    clause.push_str(" NOT NULL AUTO_INCREMENT");
                } else if !col.is_nullable {
                    clause.push_str(" NOT NULL");
                }

                if let Some(ref def_val) = col.default_value {
                    let d = def_val.trim();
                    if !d.is_empty() {
                        clause.push_str(&format!(" DEFAULT {d}"));
                    }
                }

                if let Some(ref comment) = col.comment {
                    let c = comment.trim();
                    if !c.is_empty() {
                        clause.push_str(&format!(" COMMENT '{}'", c.replace('\'', "''")));
                    }
                }

                col_clauses.push(clause);
            }

            if !pks.is_empty() {
                let pk_cols = pks
                    .iter()
                    .map(|c| quote_ident(c.name.trim(), family))
                    .collect::<Vec<_>>()
                    .join(", ");
                col_clauses.push(format!("    PRIMARY KEY ({pk_cols})"));
            }

            for idx in &def.indexes {
                let valid_idx_cols: Vec<&str> = idx
                    .columns
                    .iter()
                    .map(|c| c.trim())
                    .filter(|c| !c.is_empty())
                    .collect();
                if valid_idx_cols.is_empty() {
                    continue;
                }
                let idx_name = idx.name.trim();
                let actual_name = if idx_name.is_empty() {
                    let sanitized: Vec<String> = valid_idx_cols
                        .iter()
                        .map(|c| extract_base_column_name(c))
                        .collect();
                    format!("idx_{}_{}", table_name, sanitized.join("_"))
                } else {
                    idx_name.to_string()
                };
                let quoted_idx = quote_ident(&actual_name, family);
                let cols_str = valid_idx_cols
                    .iter()
                    .map(|c| format_index_column_expr(c, family))
                    .collect::<Vec<_>>()
                    .join(", ");
                let key_type = match idx.index_type {
                    TableIndexType::Unique => "UNIQUE KEY",
                    TableIndexType::Normal => "KEY",
                };
                col_clauses.push(format!("    {key_type} {quoted_idx} ({cols_str})"));
            }

            let mut table_suffix = " ENGINE=InnoDB DEFAULT CHARSET=utf8mb4".to_string();
            if let Some(ref t_comment) = def.comment {
                let c = t_comment.trim();
                if !c.is_empty() {
                    table_suffix.push_str(&format!(" COMMENT='{}'", c.replace('\'', "''")));
                }
            }

            let ddl = format!(
                "CREATE TABLE {quoted_table} (\n{}\n){table_suffix};",
                col_clauses.join(",\n")
            );
            Ok(ddl)
        }
    }
}

/// Parses a CREATE TABLE SQL script (supporting SQLite, PostgreSQL, and MySQL dialects)
/// into a structured `CreateTableDef`.
/// Also supports multi-statement scripts containing subsequent `COMMENT ON` or `CREATE INDEX` statements.
pub fn parse_create_table_sql(
    sql: &str,
    _default_family: DatabaseFamily,
) -> Result<CreateTableDef, String> {
    let raw = sql.trim();
    if raw.is_empty() {
        return Err("SQL statement is empty".to_string());
    }

    // Split multi-statement scripts (e.g. CREATE TABLE + COMMENT ON + CREATE INDEX)
    let statements = split_sql_statements(raw);
    let mut main_stmt = None;
    let mut extra_stmts = Vec::new();

    for stmt in statements {
        let upper = stmt.trim().to_ascii_uppercase();
        if upper.starts_with("CREATE TABLE") || upper.starts_with("CREATE TEMPORARY TABLE") || upper.starts_with("CREATE TEMP TABLE") {
            if main_stmt.is_none() {
                main_stmt = Some(stmt);
            } else {
                extra_stmts.push(stmt);
            }
        } else {
            extra_stmts.push(stmt);
        }
    }

    let Some(main_sql) = main_stmt else {
        return Err("No valid 'CREATE TABLE' statement found in input".to_string());
    };

    let mut def = parse_single_create_table_statement(&main_sql)?;

    // Process supplementary statements (PostgreSQL COMMENT ON and independent CREATE INDEX)
    for extra in extra_stmts {
        let trimmed = extra.trim();
        let upper = trimmed.to_ascii_uppercase();

        if upper.starts_with("COMMENT ON TABLE") {
            // COMMENT ON TABLE [schema.]table IS 'comment';
            if let Some(pos) = upper.find(" IS ") {
                let comment_part = &trimmed[pos + 4..].trim_end_matches(';').trim();
                if let Some(c) = extract_quoted_literal(comment_part) {
                    def.comment = Some(c);
                }
            }
        } else if upper.starts_with("COMMENT ON COLUMN") {
            // COMMENT ON COLUMN [schema.]table.column IS 'comment';
            if let Some(pos) = upper.find(" IS ") {
                let target_part = trimmed[17..pos].trim();
                let comment_part = &trimmed[pos + 4..].trim_end_matches(';').trim();
                if let Some(c) = extract_quoted_literal(comment_part) {
                    // Extract column name from dot notation (e.g. "public"."users"."id" or users.id)
                    let sub_parts: Vec<&str> = target_part.split('.').collect();
                    if let Some(last_col) = sub_parts.last() {
                        let clean_col = strip_identifier_quotes(last_col);
                        if let Some(col) = def.columns.iter_mut().find(|c| c.name.eq_ignore_ascii_case(&clean_col)) {
                            col.comment = Some(c);
                        }
                    }
                }
            }
        } else if upper.starts_with("CREATE INDEX") || upper.starts_with("CREATE UNIQUE INDEX") {
            // CREATE [UNIQUE] INDEX [name] ON [table] (col1, col2)
            let is_unique = upper.starts_with("CREATE UNIQUE INDEX");
            let after_idx = if is_unique { &trimmed[19..] } else { &trimmed[12..] }.trim();

            if let Some(on_idx) = after_idx.to_ascii_uppercase().find(" ON ") {
                let idx_name_raw = after_idx[..on_idx].trim();
                let idx_name = strip_identifier_quotes(idx_name_raw);
                let rem = &after_idx[on_idx + 4..];

                if let Some(paren_start) = rem.find('(') {
                    if let Some(paren_end) = rem.rfind(')') {
                        let cols_raw = &rem[paren_start + 1..paren_end];
                        let cols = parse_sql_column_list(cols_raw);
                        if !cols.is_empty() {
                            let mut index_def = TableIndexDef::new(idx_name, cols);
                            if is_unique {
                                index_def = index_def.unique(true);
                            }
                            def.indexes.push(index_def);
                        }
                    }
                }
            }
        }
    }

    Ok(def)
}

/// Parses the primary `CREATE TABLE ... (...)` statement body.
fn parse_single_create_table_statement(sql: &str) -> Result<CreateTableDef, String> {
    let clean = sql.trim();
    let upper = clean.to_ascii_uppercase();

    // 1. Locate start of table name
    let create_idx = upper
        .find("CREATE TABLE")
        .or_else(|| upper.find("CREATE TEMPORARY TABLE"))
        .or_else(|| upper.find("CREATE TEMP TABLE"))
        .ok_or_else(|| "Not a CREATE TABLE statement".to_string())?;

    let after_create = if upper[create_idx..].starts_with("CREATE TEMPORARY TABLE") {
        &clean[create_idx + 22..]
    } else if upper[create_idx..].starts_with("CREATE TEMP TABLE") {
        &clean[create_idx + 17..]
    } else {
        &clean[create_idx + 12..]
    }
    .trim();

    // Skip optional IF NOT EXISTS
    let after_if_not_exists = if after_create.to_ascii_uppercase().starts_with("IF NOT EXISTS") {
        after_create[13..].trim()
    } else {
        after_create
    };

    // Find the opening parenthesis of column definitions
    let open_paren_idx = after_if_not_exists
        .find('(')
        .ok_or_else(|| "Missing opening parenthesis '(' in CREATE TABLE".to_string())?;

    let table_ident = after_if_not_exists[..open_paren_idx].trim();
    let (schema_opt, table_name) = parse_qualified_identifier(table_ident)
        .ok_or_else(|| format!("Invalid table name identifier: '{table_ident}'"))?;

    // Find the matching outermost closing parenthesis
    let body_start = open_paren_idx + 1;
    let mut depth = 1;
    let chars: Vec<char> = after_if_not_exists.chars().collect();
    let mut close_paren_idx = None;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;

    let mut i = body_start;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' && !in_double_quote && !in_backtick {
            in_single_quote = !in_single_quote;
        } else if ch == '"' && !in_single_quote && !in_backtick {
            in_double_quote = !in_double_quote;
        } else if ch == '`' && !in_single_quote && !in_double_quote {
            in_backtick = !in_backtick;
        } else if !in_single_quote && !in_double_quote && !in_backtick {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    close_paren_idx = Some(i);
                    break;
                }
            }
        }
        i += 1;
    }

    let close_paren_idx = close_paren_idx
        .ok_or_else(|| "Unmatched closing parenthesis ')' in CREATE TABLE".to_string())?;

    let body_content: String = chars[body_start..close_paren_idx].iter().collect();
    let after_body: String = chars[close_paren_idx + 1..].iter().collect();

    let mut def = CreateTableDef::new(table_name).schema(schema_opt);

    // Extract table comment from options after closing parenthesis (e.g. MySQL COMMENT='...')
    if let Some(c) = extract_mysql_table_comment(&after_body) {
        def.comment = Some(c);
    }

    // Split body into comma-separated items safely
    let items = split_bracket_comma_items(&body_content);

    let mut table_pks: Vec<String> = Vec::new();

    for item in items {
        let trimmed_item = item.trim();
        if trimmed_item.is_empty() {
            continue;
        }

        let item_upper = trimmed_item.to_ascii_uppercase();

        // 1. Table-level PRIMARY KEY constraint: PRIMARY KEY (col1, col2)
        if item_upper.starts_with("PRIMARY KEY") || item_upper.starts_with("CONSTRAINT") && item_upper.contains("PRIMARY KEY") {
            if let Some(open) = trimmed_item.find('(') {
                if let Some(close) = trimmed_item.rfind(')') {
                    let cols_str = &trimmed_item[open + 1..close];
                    for col in parse_sql_column_list(cols_str) {
                        let clean_pk = extract_base_column_name(&col);
                        if !clean_pk.is_empty() {
                            table_pks.push(clean_pk);
                        }
                    }
                    continue;
                }
            }
        }

        // 2. Table-level index constraint:
        //    UNIQUE KEY [name] (col1), KEY [name] (col1), INDEX [name] (col1)
        if item_upper.starts_with("UNIQUE KEY")
            || item_upper.starts_with("UNIQUE INDEX")
            || item_upper.starts_with("KEY")
            || item_upper.starts_with("INDEX")
            || (item_upper.starts_with("CONSTRAINT") && item_upper.contains("UNIQUE"))
        {
            if let Some(idx_def) = parse_table_level_index_item(trimmed_item) {
                def.indexes.push(idx_def);
                continue;
            }
        }

        // 3. Skip standalone FOREIGN KEY or CHECK table constraints
        if item_upper.starts_with("FOREIGN KEY")
            || item_upper.starts_with("CHECK")
            || (item_upper.starts_with("CONSTRAINT") && (item_upper.contains("FOREIGN KEY") || item_upper.contains("CHECK")))
        {
            continue;
        }

        // 4. Otherwise, parse as ColumnDef
        if let Some(col_def) = parse_column_def_item(trimmed_item) {
            def.columns.push(col_def);
        }
    }

    // Apply any table-level PRIMARY KEY annotations to matching columns
    if !table_pks.is_empty() {
        for col in &mut def.columns {
            if table_pks.iter().any(|pk| pk.eq_ignore_ascii_case(&col.name)) {
                col.is_primary_key = true;
                col.is_nullable = false;
            }
        }
    }

    if def.columns.is_empty() {
        return Err("No column definitions found in CREATE TABLE".to_string());
    }

    Ok(def)
}

/// Safely splits the contents of the main CREATE TABLE parenthesis by commas,
/// respecting nested parentheses (e.g. `DECIMAL(10, 2)`) and quoted strings.
fn split_bracket_comma_items(s: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth: usize = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_backtick = false;

    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        if ch == '\'' && !in_double && !in_backtick {
            in_single = !in_single;
            current.push(ch);
        } else if ch == '"' && !in_single && !in_backtick {
            in_double = !in_double;
            current.push(ch);
        } else if ch == '`' && !in_single && !in_double {
            in_backtick = !in_backtick;
            current.push(ch);
        } else if !in_single && !in_double && !in_backtick {
            if ch == '(' {
                depth += 1;
                current.push(ch);
            } else if ch == ')' {
                depth = depth.saturating_sub(1);
                current.push(ch);
            } else if ch == ',' && depth == 0 {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    items.push(trimmed.to_string());
                }
                current.clear();
            } else {
                current.push(ch);
            }
        } else {
            current.push(ch);
        }
        i += 1;
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        items.push(trimmed.to_string());
    }

    items
}

/// Parses an individual column definition string (e.g. `"name" VARCHAR(255) NOT NULL DEFAULT 'active' COMMENT 'username'`).
fn parse_column_def_item(item: &str) -> Option<ColumnDef> {
    let clean = item.trim();
    if clean.is_empty() {
        return None;
    }

    // Split words while respecting quotes and parenthesis
    let tokens = tokenize_sql_clause(clean);
    if tokens.is_empty() {
        return None;
    }

    let col_name = strip_identifier_quotes(&tokens[0]);
    if col_name.is_empty() {
        return None;
    }

    let mut data_type_tokens = Vec::new();
    let mut is_primary_key = false;
    let mut is_nullable = true;
    let mut is_auto_increment = false;
    let mut default_value = None;
    let mut comment = None;

    let mut idx = 1;
    let token_count = tokens.len();

    // 1. Gather data type tokens until modifier keywords are met
    while idx < token_count {
        let tok = &tokens[idx];
        let tok_upper = tok.to_ascii_uppercase();

        if tok_upper == "PRIMARY"
            || tok_upper == "NOT"
            || tok_upper == "NULL"
            || tok_upper == "AUTO_INCREMENT"
            || tok_upper == "AUTOINCREMENT"
            || tok_upper == "DEFAULT"
            || tok_upper == "COMMENT"
            || tok_upper == "UNIQUE"
            || tok_upper == "REFERENCES"
            || tok_upper == "CHECK"
            || tok_upper == "COLLATE"
            || tok_upper == "GENERATED"
            || tok_upper == "AS"
        {
            break;
        }

        data_type_tokens.push(tok.clone());
        idx += 1;
    }

    let mut data_type = data_type_tokens.join(" ");
    let dt_upper = data_type.to_ascii_uppercase();

    // Check for PostgreSQL serial types or SQLite INTEGER PRIMARY KEY AUTOINCREMENT
    if dt_upper == "SERIAL" || dt_upper == "BIGSERIAL" || dt_upper == "SMALLSERIAL" {
        is_auto_increment = true;
        is_primary_key = true;
        is_nullable = false;
    }

    // 2. Parse remaining modifier tokens
    while idx < token_count {
        let tok = &tokens[idx];
        let tok_upper = tok.to_ascii_uppercase();

        if tok_upper == "PRIMARY" {
            if idx + 1 < token_count && tokens[idx + 1].eq_ignore_ascii_case("KEY") {
                is_primary_key = true;
                is_nullable = false;
                idx += 2;
                continue;
            }
        } else if tok_upper == "NOT" {
            if idx + 1 < token_count && tokens[idx + 1].eq_ignore_ascii_case("NULL") {
                is_nullable = false;
                idx += 2;
                continue;
            }
        } else if tok_upper == "NULL" {
            is_nullable = true;
            idx += 1;
            continue;
        } else if tok_upper == "AUTO_INCREMENT" || tok_upper == "AUTOINCREMENT" {
            is_auto_increment = true;
            is_primary_key = true;
            is_nullable = false;
            idx += 1;
            continue;
        } else if tok_upper == "DEFAULT" {
            if idx + 1 < token_count {
                let val_token = &tokens[idx + 1];
                default_value = Some(val_token.clone());
                idx += 2;
                continue;
            }
        } else if tok_upper == "COMMENT" {
            if idx + 1 < token_count {
                let comment_token = &tokens[idx + 1];
                comment = extract_quoted_literal(comment_token).or_else(|| Some(comment_token.clone()));
                idx += 2;
                continue;
            }
        }

        idx += 1;
    }

    // If data type is empty, default to TEXT
    if data_type.trim().is_empty() {
        data_type = "TEXT".to_string();
    }

    if is_primary_key {
        is_nullable = false;
    }

    Some(ColumnDef {
        name: col_name,
        data_type,
        is_primary_key,
        is_nullable,
        is_auto_increment,
        default_value,
        comment,
    })
}

/// Parses an inline table-level index (e.g. `UNIQUE KEY uk_name (col1, col2)` or `KEY (col1)`).
fn parse_table_level_index_item(item: &str) -> Option<TableIndexDef> {
    let clean = item.trim();

    let open_paren = clean.find('(')?;
    let close_paren = clean.rfind(')')?;
    if close_paren <= open_paren {
        return None;
    }

    let cols_str = &clean[open_paren + 1..close_paren];
    let cols = parse_sql_column_list(cols_str);
    if cols.is_empty() {
        return None;
    }

    let prefix = clean[..open_paren].trim();
    let prefix_upper = prefix.to_ascii_uppercase();
    let is_unique = prefix_upper.starts_with("UNIQUE");

    let tokens: Vec<&str> = prefix.split_whitespace().collect();
    let mut idx_name = String::new();

    // Look for identifier after KEY / INDEX
    for (i, &t) in tokens.iter().enumerate() {
        let tu = t.to_ascii_uppercase();
        if (tu == "KEY" || tu == "INDEX" || tu == "UNIQUE") && i + 1 < tokens.len() {
            let candidate = strip_identifier_quotes(tokens[i + 1]);
            if !candidate.is_empty()
                && !candidate.eq_ignore_ascii_case("KEY")
                && !candidate.eq_ignore_ascii_case("INDEX")
            {
                idx_name = candidate;
                break;
            }
        }
    }

    let mut index_def = TableIndexDef::new(idx_name, cols);
    if is_unique {
        index_def = index_def.unique(true);
    }
    Some(index_def)
}

/// Tokenizes a SQL clause while preserving quoted strings and parenthesized groups (like VARCHAR(255)).
fn tokenize_sql_clause(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut in_backtick = false;
    let mut paren_depth: usize = 0;

    for ch in s.chars() {
        if ch == '\'' && !in_double && !in_backtick {
            in_single = !in_single;
            current.push(ch);
        } else if ch == '"' && !in_single && !in_backtick {
            in_double = !in_double;
            current.push(ch);
        } else if ch == '`' && !in_single && !in_double {
            in_backtick = !in_backtick;
            current.push(ch);
        } else if !in_single && !in_double && !in_backtick {
            if ch == '(' {
                paren_depth += 1;
                current.push(ch);
            } else if ch == ')' {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            } else if ch.is_whitespace() && paren_depth == 0 {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            } else {
                current.push(ch);
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Extracts the literal content from a single-quoted string literal (e.g. `'hello world'` -> `hello world`),
/// un-escaping `''` and `\'`.
fn extract_quoted_literal(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        Some(inner.replace("''", "'").replace("\\'", "'"))
    } else {
        None
    }
}

/// Extracts MySQL table comment from table options (e.g. `ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='Table info'`).
fn extract_mysql_table_comment(options: &str) -> Option<String> {
    let upper = options.to_ascii_uppercase();
    let comment_pos = upper.find("COMMENT")?;
    let rem = options[comment_pos + 7..].trim();
    let after_eq = if rem.starts_with('=') { rem[1..].trim() } else { rem };
    extract_quoted_literal(after_eq)
}

/// Builds an INSERT INTO SQL template statement with dialect identifier quoting and parameter placeholders.
/// Automatically omits auto-increment / serial primary key columns when other writable columns exist.
pub fn build_insert_template(
    table: &TableInfo,
    columns: &[ColumnInfo],
    family: DatabaseFamily,
) -> String {
    let qualified = table.qualified_name(family);
    if columns.is_empty() {
        return match family {
            DatabaseFamily::MySql => format!("INSERT INTO {qualified} () VALUES ();"),
            _ => format!("INSERT INTO {qualified} DEFAULT VALUES;"),
        };
    }

    // Filter columns: omit auto-increment columns if at least one non-auto column exists
    let has_non_auto = columns.iter().any(|c| !c.is_auto_increment);
    let target_cols: Vec<&ColumnInfo> = if has_non_auto {
        columns.iter().filter(|c| !c.is_auto_increment).collect()
    } else {
        columns.iter().collect()
    };

    let col_names: Vec<String> = target_cols
        .iter()
        .map(|c| quote_ident(&c.name, family))
        .collect();

    let placeholders: Vec<String> = target_cols
        .iter()
        .map(|c| {
            if let Some(ref def) = c.default_value {
                let d = def.trim();
                if !d.is_empty() {
                    return d.to_string();
                }
            }
            "?".to_string()
        })
        .collect();

    let cols_str = col_names.join(", ");
    let vals_str = placeholders.join(", ");

    if cols_str.len() > 60 || target_cols.len() > 5 {
        let indented_cols = col_names
            .iter()
            .map(|c| format!("    {c}"))
            .collect::<Vec<_>>()
            .join(",\n");
        let indented_vals = placeholders
            .iter()
            .map(|v| format!("    {v}"))
            .collect::<Vec<_>>()
            .join(",\n");
        format!("INSERT INTO {qualified} (\n{indented_cols}\n) VALUES (\n{indented_vals}\n);")
    } else {
        format!("INSERT INTO {qualified} ({cols_str}) VALUES ({vals_str});")
    }
}

/// Target column specification during schema alteration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlterColumnTarget {
    /// Original column name if this target column was derived from an existing column.
    /// `None` indicates a newly added column.
    pub original_name: Option<String>,
    /// Updated column definition.
    pub definition: ColumnDef,
}

impl AlterColumnTarget {
    pub fn new(original_name: Option<String>, definition: ColumnDef) -> Self {
        Self {
            original_name,
            definition,
        }
    }
}

/// Represents an individual column-level change in an ALTER TABLE plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnAlteration {
    AddColumn {
        column: ColumnDef,
        after: Option<String>,
    },
    ModifyColumn {
        old_column: ColumnInfo,
        new_column: ColumnDef,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
        new_column: ColumnDef,
    },
    DropColumn {
        name: String,
    },
}

/// Structured plan of table alteration statements and metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct AlterTablePlan {
    pub table_name: String,
    pub schema_name: Option<String>,
    pub family: DatabaseFamily,
    pub alterations: Vec<ColumnAlteration>,
    pub statements: Vec<String>,
    pub warnings: Vec<String>,
    pub full_script: String,
}

impl AlterTablePlan {
    /// Converts this alteration plan into a standard SqlReviewPlan for execution review.
    pub fn to_review_plan(&self) -> SqlReviewPlan {
        let inserts_count = self
            .alterations
            .iter()
            .filter(|a| matches!(a, ColumnAlteration::AddColumn { .. }))
            .count();
        let updates_count = self
            .alterations
            .iter()
            .filter(|a| {
                matches!(
                    a,
                    ColumnAlteration::ModifyColumn { .. } | ColumnAlteration::RenameColumn { .. }
                )
            })
            .count();
        let deletes_count = self
            .alterations
            .iter()
            .filter(|a| matches!(a, ColumnAlteration::DropColumn { .. }))
            .count();

        SqlReviewPlan {
            table_name: self.table_name.clone(),
            schema_name: self.schema_name.clone(),
            database_family: self.family,
            inserts_count,
            updates_count,
            deletes_count,
            has_primary_key: true,
            primary_keys: vec![],
            warnings: self.warnings.clone(),
            statements: self.statements.clone(),
            full_script: self.full_script.clone(),
        }
    }
}

/// Generates dialect-safe ALTER TABLE migration scripts comparing baseline schema against target columns.
pub fn generate_alter_table_plan(
    table_name: &str,
    schema_name: Option<&str>,
    family: DatabaseFamily,
    original_cols: &[ColumnInfo],
    target_cols: &[AlterColumnTarget],
) -> AlterTablePlan {
    let mut warnings = Vec::new();
    let mut statements = Vec::new();
    let mut alterations = Vec::new();

    let qualified_table = match schema_name {
        Some(s) if !s.trim().is_empty() && !s.eq_ignore_ascii_case("main") => {
            format!("{}.{}", quote_ident(s, family), quote_ident(table_name, family))
        }
        _ => quote_ident(table_name, family),
    };

    // 1. Detect dropped columns (present in original_cols, but absent from target_cols)
    let retained_orig_names: std::collections::HashSet<String> = target_cols
        .iter()
        .filter_map(|t| t.original_name.as_ref().map(|s| s.trim().to_lowercase()))
        .collect();

    for orig in original_cols {
        if !retained_orig_names.contains(&orig.name.trim().to_lowercase()) {
            alterations.push(ColumnAlteration::DropColumn {
                name: orig.name.clone(),
            });
        }
    }

    // 2. Detect added, renamed, or modified columns
    let mut prev_col_name: Option<String> = None;
    for target in target_cols {
        let col = &target.definition;
        let col_name = col.name.trim();
        if col_name.is_empty() {
            continue;
        }

        match &target.original_name {
            None => {
                // Brand new column
                alterations.push(ColumnAlteration::AddColumn {
                    column: col.clone(),
                    after: prev_col_name.clone(),
                });
            }
            Some(old_name) => {
                let orig_opt = original_cols
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(old_name));

                if let Some(orig) = orig_opt {
                    let is_renamed = !orig.name.eq_ignore_ascii_case(col_name);
                    let type_changed = !orig.data_type.eq_ignore_ascii_case(col.data_type.trim());
                    let null_changed = orig.is_nullable != col.is_nullable;
                    let def_changed = normalize_opt_str(orig.default_value.as_deref())
                        != normalize_opt_str(col.default_value.as_deref());
                    let comment_changed = normalize_opt_str(orig.description.as_deref())
                        != normalize_opt_str(col.comment.as_deref());
                    let pk_changed = orig.is_primary_key != col.is_primary_key;

                    if is_renamed {
                        alterations.push(ColumnAlteration::RenameColumn {
                            old_name: orig.name.clone(),
                            new_name: col_name.to_string(),
                            new_column: col.clone(),
                        });
                    } else if type_changed || null_changed || def_changed || comment_changed || pk_changed {
                        alterations.push(ColumnAlteration::ModifyColumn {
                            old_column: orig.clone(),
                            new_column: col.clone(),
                        });
                    }
                } else {
                    // Fallback to add column if original metadata was missing
                    alterations.push(ColumnAlteration::AddColumn {
                        column: col.clone(),
                        after: prev_col_name.clone(),
                    });
                }
            }
        }
        prev_col_name = Some(col_name.to_string());
    }

    if alterations.is_empty() {
        return AlterTablePlan {
            table_name: table_name.to_string(),
            schema_name: schema_name.map(|s| s.to_string()),
            family,
            alterations,
            statements: vec!["-- No changes detected in table columns".to_string()],
            warnings,
            full_script: "-- No changes detected in table columns".to_string(),
        };
    }

    // 3. Generate dialect-specific statements
    match family {
        DatabaseFamily::MySql => {
            for alt in &alterations {
                match alt {
                    ColumnAlteration::DropColumn { name } => {
                        statements.push(format!(
                            "ALTER TABLE {qualified_table} DROP COLUMN {};",
                            quote_ident(name, family)
                        ));
                    }
                    ColumnAlteration::AddColumn { column, after } => {
                        let mut clause = format!(
                            "ADD COLUMN {} {}",
                            quote_ident(&column.name, family),
                            column.data_type.trim()
                        );
                        if column.is_auto_increment {
                            clause.push_str(" NOT NULL AUTO_INCREMENT");
                        } else if !column.is_nullable {
                            clause.push_str(" NOT NULL");
                        }
                        if let Some(ref d) = column.default_value {
                            let dt = d.trim();
                            if !dt.is_empty() {
                                clause.push_str(&format!(" DEFAULT {dt}"));
                            }
                        }
                        if let Some(ref c) = column.comment {
                            let ct = c.trim();
                            if !ct.is_empty() {
                                clause.push_str(&format!(" COMMENT '{}'", ct.replace('\'', "''")));
                            }
                        }
                        if let Some(prev) = after {
                            clause.push_str(&format!(" AFTER {}", quote_ident(prev, family)));
                        }
                        statements.push(format!("ALTER TABLE {qualified_table} {clause};"));
                    }
                    ColumnAlteration::ModifyColumn {
                        old_column: _,
                        new_column,
                    } => {
                        let mut clause = format!(
                            "MODIFY COLUMN {} {}",
                            quote_ident(&new_column.name, family),
                            new_column.data_type.trim()
                        );
                        if new_column.is_auto_increment {
                            clause.push_str(" NOT NULL AUTO_INCREMENT");
                        } else if !new_column.is_nullable {
                            clause.push_str(" NOT NULL");
                        }
                        if let Some(ref d) = new_column.default_value {
                            let dt = d.trim();
                            if !dt.is_empty() {
                                clause.push_str(&format!(" DEFAULT {dt}"));
                            }
                        }
                        if let Some(ref c) = new_column.comment {
                            let ct = c.trim();
                            if !ct.is_empty() {
                                clause.push_str(&format!(" COMMENT '{}'", ct.replace('\'', "''")));
                            }
                        }
                        statements.push(format!("ALTER TABLE {qualified_table} {clause};"));
                    }
                    ColumnAlteration::RenameColumn {
                        old_name,
                        new_name,
                        new_column,
                    } => {
                        let mut clause = format!(
                            "CHANGE COLUMN {} {} {}",
                            quote_ident(old_name, family),
                            quote_ident(new_name, family),
                            new_column.data_type.trim()
                        );
                        if new_column.is_auto_increment {
                            clause.push_str(" NOT NULL AUTO_INCREMENT");
                        } else if !new_column.is_nullable {
                            clause.push_str(" NOT NULL");
                        }
                        if let Some(ref d) = new_column.default_value {
                            let dt = d.trim();
                            if !dt.is_empty() {
                                clause.push_str(&format!(" DEFAULT {dt}"));
                            }
                        }
                        if let Some(ref c) = new_column.comment {
                            let ct = c.trim();
                            if !ct.is_empty() {
                                clause.push_str(&format!(" COMMENT '{}'", ct.replace('\'', "''")));
                            }
                        }
                        statements.push(format!("ALTER TABLE {qualified_table} {clause};"));
                    }
                }
            }
        }

        DatabaseFamily::Postgres => {
            for alt in &alterations {
                match alt {
                    ColumnAlteration::DropColumn { name } => {
                        statements.push(format!(
                            "ALTER TABLE {qualified_table} DROP COLUMN {};",
                            quote_ident(name, family)
                        ));
                    }
                    ColumnAlteration::AddColumn { column, .. } => {
                        let mut clause = format!(
                            "ADD COLUMN {} {}",
                            quote_ident(&column.name, family),
                            column.data_type.trim()
                        );
                        if let Some(ref d) = column.default_value {
                            let dt = d.trim();
                            if !dt.is_empty() {
                                clause.push_str(&format!(" DEFAULT {dt}"));
                            }
                        }
                        if !column.is_nullable {
                            clause.push_str(" NOT NULL");
                        }
                        statements.push(format!("ALTER TABLE {qualified_table} {clause};"));

                        if let Some(ref c) = column.comment {
                            let ct = c.trim();
                            if !ct.is_empty() {
                                statements.push(format!(
                                    "COMMENT ON COLUMN {qualified_table}.{} IS '{}';",
                                    quote_ident(&column.name, family),
                                    ct.replace('\'', "''")
                                ));
                            }
                        }
                    }
                    ColumnAlteration::RenameColumn {
                        old_name,
                        new_name,
                        new_column,
                    } => {
                        statements.push(format!(
                            "ALTER TABLE {qualified_table} RENAME COLUMN {} TO {};",
                            quote_ident(old_name, family),
                            quote_ident(new_name, family)
                        ));
                        // Check if type also needs alteration
                        let old_opt = original_cols.iter().find(|c| c.name == *old_name);
                        if let Some(old) = old_opt {
                            if !old.data_type.eq_ignore_ascii_case(new_column.data_type.trim()) {
                                statements.push(format!(
                                    "ALTER TABLE {qualified_table} ALTER COLUMN {} TYPE {};",
                                    quote_ident(new_name, family),
                                    new_column.data_type.trim()
                                ));
                            }
                        }
                    }
                    ColumnAlteration::ModifyColumn {
                        old_column,
                        new_column,
                    } => {
                        let quoted_col = quote_ident(&new_column.name, family);

                        // 1. Data type change
                        if !old_column.data_type.eq_ignore_ascii_case(new_column.data_type.trim()) {
                            statements.push(format!(
                                "ALTER TABLE {qualified_table} ALTER COLUMN {quoted_col} TYPE {};",
                                new_column.data_type.trim()
                            ));
                        }

                        // 2. Nullability change
                        if old_column.is_nullable != new_column.is_nullable {
                            if new_column.is_nullable {
                                statements.push(format!(
                                    "ALTER TABLE {qualified_table} ALTER COLUMN {quoted_col} DROP NOT NULL;"
                                ));
                            } else {
                                statements.push(format!(
                                    "ALTER TABLE {qualified_table} ALTER COLUMN {quoted_col} SET NOT NULL;"
                                ));
                            }
                        }

                        // 3. Default value change
                        let old_def = normalize_opt_str(old_column.default_value.as_deref());
                        let new_def = normalize_opt_str(new_column.default_value.as_deref());
                        if old_def != new_def {
                            if let Some(d) = new_def {
                                statements.push(format!(
                                    "ALTER TABLE {qualified_table} ALTER COLUMN {quoted_col} SET DEFAULT {d};"
                                ));
                            } else {
                                statements.push(format!(
                                    "ALTER TABLE {qualified_table} ALTER COLUMN {quoted_col} DROP DEFAULT;"
                                ));
                            }
                        }

                        // 4. Column comment change
                        let old_comment = normalize_opt_str(old_column.description.as_deref());
                        let new_comment = normalize_opt_str(new_column.comment.as_deref());
                        if old_comment != new_comment {
                            let c_str = new_comment.unwrap_or_default();
                            statements.push(format!(
                                "COMMENT ON COLUMN {qualified_table}.{quoted_col} IS '{}';",
                                c_str.replace('\'', "''")
                            ));
                        }
                    }
                }
            }
        }

        DatabaseFamily::Sqlite => {
            // Check if changes only contain AddColumn
            let only_add_columns = alterations.iter().all(|a| matches!(a, ColumnAlteration::AddColumn { .. }));

            if only_add_columns {
                for alt in &alterations {
                    if let ColumnAlteration::AddColumn { column, .. } = alt {
                        let mut clause = format!(
                            "ADD COLUMN {} {}",
                            quote_ident(&column.name, family),
                            column.data_type.trim()
                        );
                        if let Some(ref d) = column.default_value {
                            let dt = d.trim();
                            if !dt.is_empty() {
                                clause.push_str(&format!(" DEFAULT {dt}"));
                            }
                        }
                        if !column.is_nullable {
                            if column.default_value.is_none() {
                                warnings.push(format!(
                                    "Adding NOT NULL column '{}' without a DEFAULT value in SQLite will fail if table contains existing rows.",
                                    column.name
                                ));
                            }
                            clause.push_str(" NOT NULL");
                        }
                        statements.push(format!("ALTER TABLE {qualified_table} {clause};"));
                    }
                }
            } else {
                // Table recreation pattern for complex SQLite schema changes
                warnings.push(format!(
                    "SQLite requires recreating table '{table_name}' to apply column modifications/deletions. Existing triggers or indexes may need manual recreation."
                ));

                let temp_table_name = format!("{table_name}_new_migration");
                let quoted_temp = quote_ident(&temp_table_name, family);

                // Build new column list
                let mut new_col_defs = Vec::new();
                for target in target_cols {
                    let col = &target.definition;
                    let mut clause = format!(
                        "    {} {}",
                        quote_ident(&col.name, family),
                        if col.data_type.trim().is_empty() { "TEXT" } else { col.data_type.trim() }
                    );
                    if col.is_primary_key {
                        if col.is_auto_increment {
                            clause = format!("    {} INTEGER PRIMARY KEY AUTOINCREMENT", quote_ident(&col.name, family));
                        } else {
                            clause.push_str(" PRIMARY KEY");
                        }
                    } else if !col.is_nullable {
                        clause.push_str(" NOT NULL");
                    }
                    if let Some(ref d) = col.default_value {
                        let dt = d.trim();
                        if !dt.is_empty() {
                            clause.push_str(&format!(" DEFAULT {dt}"));
                        }
                    }
                    new_col_defs.push(clause);
                }

                // Identify common columns for data migration
                let mut select_pairs = Vec::new();
                for target in target_cols {
                    if let Some(ref orig_name) = target.original_name {
                        if original_cols.iter().any(|c| c.name.eq_ignore_ascii_case(orig_name)) {
                            select_pairs.push((
                                quote_ident(&target.definition.name, family),
                                quote_ident(orig_name, family),
                            ));
                        }
                    }
                }

                let create_temp_stmt = format!(
                    "CREATE TABLE {quoted_temp} (\n{}\n);",
                    new_col_defs.join(",\n")
                );

                statements.push("PRAGMA foreign_keys = OFF;".to_string());
                statements.push(create_temp_stmt);

                if !select_pairs.is_empty() {
                    let dest_cols = select_pairs
                        .iter()
                        .map(|p| p.0.clone())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let src_cols = select_pairs
                        .iter()
                        .map(|p| p.1.clone())
                        .collect::<Vec<_>>()
                        .join(", ");
                    statements.push(format!(
                        "INSERT INTO {quoted_temp} ({dest_cols})\nSELECT {src_cols} FROM {qualified_table};"
                    ));
                }

                statements.push(format!("DROP TABLE {qualified_table};"));
                statements.push(format!(
                    "ALTER TABLE {quoted_temp} RENAME TO {};",
                    quote_ident(table_name, family)
                ));
                statements.push("PRAGMA foreign_keys = ON;".to_string());
            }
        }
    }

    let full_script = build_transaction_script(family, &statements);

    AlterTablePlan {
        table_name: table_name.to_string(),
        schema_name: schema_name.map(|s| s.to_string()),
        family,
        alterations,
        statements,
        warnings,
        full_script,
    }
}

fn normalize_opt_str(s: Option<&str>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::ColumnInfo;

    fn sample_columns() -> Vec<ColumnInfo> {
        vec![
            ColumnInfo {
                name: "id".into(),
                data_type: "INTEGER".into(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: true,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "name".into(),
                data_type: "TEXT".into(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "note".into(),
                data_type: "TEXT".into(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ]
    }

    #[test]
    fn test_generate_update_and_delete_with_primary_key() {
        let cols = sample_columns();
        let grid_cols = vec!["id".to_string(), "name".to_string(), "note".to_string()];
        let orig_rows = vec![
            vec![
                QueryValue::Int(1),
                QueryValue::String("Alice".into()),
                QueryValue::Null,
            ],
            vec![
                QueryValue::Int(2),
                QueryValue::String("Bob".into()),
                QueryValue::String("Hello".into()),
            ],
        ];

        let mut cs = GridChangeset::new();
        // Update row 0's name
        cs.set_cell_value(
            0,
            1,
            "name".into(),
            QueryValue::String("Alice".into()),
            QueryValue::String("Alice Smith".into()),
        );
        // Delete row 1
        cs.mark_row_deleted(1, &orig_rows[1]);

        let plan = generate_review_plan(
            "users",
            Some("public"),
            DatabaseFamily::Postgres,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        assert!(plan.has_primary_key);
        assert_eq!(plan.primary_keys, vec!["id"]);
        assert_eq!(plan.updates_count, 1);
        assert_eq!(plan.deletes_count, 1);
        assert!(plan.warnings.is_empty());

        let script = plan.full_script;
        assert!(script.starts_with("BEGIN;"));
        assert!(script.ends_with("COMMIT;"));
        assert!(script.contains("UPDATE \"public\".\"users\""));
        assert!(script.contains("SET \"name\" = 'Alice Smith'"));
        assert!(script.contains("WHERE \"id\" = 1;"));
        assert!(script.contains("DELETE FROM \"public\".\"users\""));
        assert!(script.contains("WHERE \"id\" = 2;"));
    }

    #[test]
    fn test_generate_review_plan_without_primary_key_fallback() {
        let cols = vec![
            ColumnInfo {
                name: "tag".into(),
                data_type: "TEXT".into(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "active".into(),
                data_type: "INTEGER".into(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];
        let grid_cols = vec!["tag".to_string(), "active".to_string()];
        let orig_rows = vec![vec![QueryValue::Null, QueryValue::Int(1)]];

        let mut cs = GridChangeset::new();
        cs.set_cell_value(
            0,
            0,
            "tag".into(),
            QueryValue::Null,
            QueryValue::String("dev's test".into()),
        );

        let plan = generate_review_plan(
            "logs",
            None,
            DatabaseFamily::Sqlite,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        assert!(!plan.has_primary_key);
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("no primary key"));

        let script = plan.full_script;
        assert!(script.contains("UPDATE \"logs\""));
        assert!(script.contains("SET \"tag\" = 'dev''s test'"));
        // WHERE clause matches all columns including NULL handling
        assert!(script.contains("WHERE \"tag\" IS NULL AND \"active\" = 1;"));
    }

    #[test]
    fn test_mysql_transaction_and_escaping() {
        let cols = sample_columns();
        let grid_cols = vec!["id".to_string(), "name".to_string(), "note".to_string()];
        let orig_rows = vec![vec![
            QueryValue::Int(10),
            QueryValue::String("O'Reilly".into()),
            QueryValue::Null,
        ]];

        let mut cs = GridChangeset::new();
        cs.set_cell_value(
            0,
            1,
            "name".into(),
            QueryValue::String("O'Reilly".into()),
            QueryValue::String("O'Reilly\\New".into()),
        );

        let plan = generate_review_plan(
            "authors",
            None,
            DatabaseFamily::MySql,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        let script = plan.full_script;
        assert!(script.starts_with("START TRANSACTION;"));
        assert!(script.contains("UPDATE `authors`"));
        assert!(script.contains("SET `name` = 'O\\'Reilly\\\\New'"));
        assert!(script.contains("WHERE `id` = 10;"));
    }

    #[test]
    fn test_insert_into_generation_sqlite_and_auto_increment() {
        let cols = sample_columns();
        let grid_cols = vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
            "age".to_string(),
            "active".to_string(),
        ];
        let orig_rows = vec![];

        let mut cs = GridChangeset::new();
        // Row 1: id is Null (auto increment) -> should omit id
        cs.add_inserted_row(
            vec![
                QueryValue::Null,
                QueryValue::String("David".into()),
                QueryValue::String("david@example.com".into()),
                QueryValue::Int(32),
                QueryValue::Int(1),
            ],
            crate::db::changeset::InsertAnchor::default(),
        );
        // Row 2: id is explicitly specified -> should include id
        cs.add_inserted_row(
            vec![
                QueryValue::Int(99),
                QueryValue::String("Eve".into()),
                QueryValue::Null,
                QueryValue::Null,
                QueryValue::Int(0),
            ],
            crate::db::changeset::InsertAnchor::default(),
        );

        let plan = generate_review_plan(
            "users",
            None,
            DatabaseFamily::Sqlite,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        assert_eq!(plan.inserts_count, 2);
        assert_eq!(plan.updates_count, 0);
        assert_eq!(plan.deletes_count, 0);

        let script = plan.full_script;
        assert!(script.starts_with("BEGIN;"));
        assert!(script.contains("INSERT INTO \"users\" (\"name\", \"email\", \"age\", \"active\") VALUES ('David', 'david@example.com', 32, 1);"));
        assert!(script.contains("INSERT INTO \"users\" (\"id\", \"name\", \"email\", \"age\", \"active\") VALUES (99, 'Eve', NULL, NULL, 0);"));
        assert!(script.ends_with("COMMIT;"));
    }

    #[test]
    fn test_insert_into_generation_mysql_dialect() {
        let cols = sample_columns();
        let grid_cols = vec!["id".to_string(), "name".to_string()];
        let orig_rows = vec![];

        let mut cs = GridChangeset::new();
        cs.add_inserted_row(
            vec![
                QueryValue::String("<auto>".into()),
                QueryValue::String("Frank's \"Gadgets\"".into()),
            ],
            crate::db::changeset::InsertAnchor::default(),
        );

        let plan = generate_review_plan(
            "products",
            Some("shop_db"),
            DatabaseFamily::MySql,
            &cols,
            &grid_cols,
            &orig_rows,
            &cs,
        );

        assert_eq!(plan.inserts_count, 1);
        let script = plan.full_script;
        assert!(script.starts_with("START TRANSACTION;"));
        assert!(script.contains(
            "INSERT INTO `shop_db`.`products` (`name`) VALUES ('Frank\\'s \"Gadgets\"');"
        ));
    }

    #[test]
    fn test_multiline_insert_formatting_for_wide_tables() {
        let cols: Vec<ColumnInfo> = (1..=10)
            .map(|i| ColumnInfo {
                name: format!("col_{i}"),
                data_type: "VARCHAR(255)".into(),
                is_nullable: true,
                is_primary_key: i == 1,
                is_auto_increment: false,
                default_value: None,
                description: None,
            })
            .collect();
        let grid_cols: Vec<String> = (1..=10).map(|i| format!("col_{i}")).collect();
        let mut cs = GridChangeset::new();
        cs.add_inserted_row(
            vec![
                QueryValue::Int(1),
                QueryValue::String("very long descriptive text title".into()),
                QueryValue::String("detailed information content goes here".into()),
                QueryValue::String("value_4".into()),
                QueryValue::String("value_5".into()),
                QueryValue::String("value_6".into()),
                QueryValue::String("value_7".into()),
                QueryValue::String("value_8".into()),
                QueryValue::String("value_9".into()),
                QueryValue::String("value_10".into()),
            ],
            crate::db::changeset::InsertAnchor::default(),
        );

        let plan = generate_review_plan(
            "test_case",
            Some("skill_up_web"),
            DatabaseFamily::MySql,
            &cols,
            &grid_cols,
            &[],
            &cs,
        );

        let script = plan.full_script;
        assert!(script.contains("INSERT INTO `skill_up_web`.`test_case` ("));
        // Check that columns and values are indented across multiple lines
        assert!(script.contains("    `col_1`,\n    `col_2`,"));
        assert!(script.contains("    1,\n    'very long descriptive text title',"));
    }

    #[test]
    fn test_extract_table_from_sql_queries() {
        // MySQL with schema
        let res = extract_table_from_sql("SELECT * FROM `skill_up_web`.`test_case` LIMIT 100;");
        assert_eq!(res, Some((Some("skill_up_web".into()), "test_case".into())));

        // Postgres with double quotes
        let res = extract_table_from_sql(
            "SELECT id, name FROM \"public\".\"users\" WHERE active = true;",
        );
        assert_eq!(res, Some((Some("public".into()), "users".into())));

        // SQLite simple table
        let res = extract_table_from_sql("SELECT * FROM test_case;");
        assert_eq!(res, Some((None, "test_case".into())));

        // MS SQL brackets
        let res = extract_table_from_sql("SELECT * FROM [dbo].[Customers]");
        assert_eq!(res, Some((Some("dbo".into()), "Customers".into())));

        // Lowercase and whitespace
        let res = extract_table_from_sql("   select  id  from   accounts   where id = 10 ");
        assert_eq!(res, Some((None, "accounts".into())));

        // INSERT statement
        let res = extract_table_from_sql("INSERT INTO `orders` (id, amount) VALUES (1, 99.9);");
        assert_eq!(res, Some((None, "orders".into())));

        // UPDATE statement
        let res = extract_table_from_sql("UPDATE `mydb`.`items` SET status = 'done';");
        assert_eq!(res, Some((Some("mydb".into()), "items".into())));

        // Non-table queries
        assert_eq!(extract_table_from_sql("SELECT 1 + 1;"), None);
        assert_eq!(extract_table_from_sql("SHOW DATABASES;"), None);
    }

    #[test]
    fn test_generate_create_table_sql_sqlite() {
        let def = CreateTableDef::new("users")
            .column(ColumnDef::new("id", "INTEGER").auto_increment(true))
            .column(ColumnDef::new("username", "TEXT").nullable(false))
            .column(ColumnDef::new("email", "TEXT").nullable(false))
            .column(ColumnDef::new("status", "TEXT").default_value(Some("'active'".into())))
            .column(
                ColumnDef::new("created_at", "DATETIME")
                    .default_value(Some("CURRENT_TIMESTAMP".into())),
            );

        let ddl = generate_create_table_sql(&def, DatabaseFamily::Sqlite).unwrap();
        assert!(ddl.contains("CREATE TABLE \"users\" ("));
        assert!(ddl.contains("    \"id\" INTEGER PRIMARY KEY AUTOINCREMENT,"));
        assert!(ddl.contains("    \"username\" TEXT NOT NULL,"));
        assert!(ddl.contains("    \"email\" TEXT NOT NULL,"));
        assert!(ddl.contains("    \"status\" TEXT DEFAULT 'active',"));
        assert!(ddl.contains("    \"created_at\" DATETIME DEFAULT CURRENT_TIMESTAMP"));
    }

    #[test]
    fn test_generate_create_table_sql_sqlite_composite_pk() {
        let def = CreateTableDef::new("order_items")
            .column(ColumnDef::new("order_id", "INTEGER").primary_key(true))
            .column(ColumnDef::new("item_id", "INTEGER").primary_key(true))
            .column(ColumnDef::new("quantity", "INTEGER").default_value(Some("1".into())));

        let ddl = generate_create_table_sql(&def, DatabaseFamily::Sqlite).unwrap();
        assert!(ddl.contains("CREATE TABLE \"order_items\" ("));
        assert!(ddl.contains("    \"order_id\" INTEGER NOT NULL,"));
        assert!(ddl.contains("    \"item_id\" INTEGER NOT NULL,"));
        assert!(ddl.contains("    PRIMARY KEY (\"order_id\", \"item_id\")"));
    }

    #[test]
    fn test_generate_create_table_sql_postgres() {
        let def = CreateTableDef::new("customers")
            .schema(Some("public".into()))
            .comment(Some("Customer records".into()))
            .column(
                ColumnDef::new("id", "BIGINT")
                    .auto_increment(true)
                    .comment(Some("Unique ID".into())),
            )
            .column(ColumnDef::new("name", "VARCHAR(255)").nullable(false))
            .column(ColumnDef::new("balance", "NUMERIC(10,2)").default_value(Some("0.00".into())));

        let ddl = generate_create_table_sql(&def, DatabaseFamily::Postgres).unwrap();
        assert!(ddl.contains("CREATE TABLE \"public\".\"customers\" ("));
        assert!(ddl.contains("    \"id\" BIGSERIAL PRIMARY KEY,"));
        assert!(ddl.contains("    \"name\" VARCHAR(255) NOT NULL,"));
        assert!(ddl.contains("    \"balance\" NUMERIC(10,2) DEFAULT 0.00"));
        assert!(ddl.contains("COMMENT ON TABLE \"public\".\"customers\" IS 'Customer records';"));
        assert!(ddl.contains("COMMENT ON COLUMN \"public\".\"customers\".\"id\" IS 'Unique ID';"));
    }

    #[test]
    fn test_generate_create_table_sql_mysql() {
        let def = CreateTableDef::new("products")
            .schema(Some("shop_db".into()))
            .comment(Some("Catalog table".into()))
            .column(ColumnDef::new("id", "INT").auto_increment(true))
            .column(
                ColumnDef::new("title", "VARCHAR(255)")
                    .nullable(false)
                    .comment(Some("Item name".into())),
            )
            .column(ColumnDef::new("price", "DECIMAL(10,2)").default_value(Some("0.0".into())));

        let ddl = generate_create_table_sql(&def, DatabaseFamily::MySql).unwrap();
        assert!(ddl.contains("CREATE TABLE `shop_db`.`products` ("));
        assert!(ddl.contains("    `id` INT NOT NULL AUTO_INCREMENT,"));
        assert!(ddl.contains("    `title` VARCHAR(255) NOT NULL COMMENT 'Item name',"));
        assert!(ddl.contains("    `price` DECIMAL(10,2) DEFAULT 0.0"));
        assert!(ddl.contains("    PRIMARY KEY (`id`)"));
        assert!(ddl.contains("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='Catalog table';"));
    }

    #[test]
    fn test_generate_create_table_sql_with_indexes() {
        let def = CreateTableDef::new("accounts")
            .schema(Some("core".into()))
            .column(ColumnDef::new("id", "BIGINT").auto_increment(true))
            .column(ColumnDef::new("email", "VARCHAR(255)").nullable(false))
            .column(ColumnDef::new("tenant_id", "INT").nullable(false))
            .column(ColumnDef::new("status", "VARCHAR(50)").default_value(Some("'active'".into())))
            .index(TableIndexDef::new("uk_accounts_email", vec!["email".into()]).unique(true))
            .index(TableIndexDef::new(
                "",
                vec!["tenant_id".into(), "status".into()],
            )); // Auto name idx_accounts_tenant_id_status

        // SQLite
        let sqlite_ddl = generate_create_table_sql(&def, DatabaseFamily::Sqlite).unwrap();
        assert!(sqlite_ddl.contains("CREATE TABLE \"core\".\"accounts\" ("));
        assert!(sqlite_ddl.contains(
            "CREATE UNIQUE INDEX \"uk_accounts_email\" ON \"core\".\"accounts\" (\"email\");"
        ));
        assert!(sqlite_ddl.contains("CREATE INDEX \"idx_accounts_tenant_id_status\" ON \"core\".\"accounts\" (\"tenant_id\", \"status\");"));

        // PostgreSQL
        let pg_ddl = generate_create_table_sql(&def, DatabaseFamily::Postgres).unwrap();
        assert!(pg_ddl.contains("CREATE TABLE \"core\".\"accounts\" ("));
        assert!(pg_ddl.contains(
            "CREATE UNIQUE INDEX \"uk_accounts_email\" ON \"core\".\"accounts\" (\"email\");"
        ));
        assert!(pg_ddl.contains("CREATE INDEX \"idx_accounts_tenant_id_status\" ON \"core\".\"accounts\" (\"tenant_id\", \"status\");"));

        // MySQL
        let mysql_ddl = generate_create_table_sql(&def, DatabaseFamily::MySql).unwrap();
        assert!(mysql_ddl.contains("CREATE TABLE `core`.`accounts` ("));
        assert!(mysql_ddl.contains("    UNIQUE KEY `uk_accounts_email` (`email`),"));
        assert!(
            mysql_ddl.contains("    KEY `idx_accounts_tenant_id_status` (`tenant_id`, `status`)")
        );
    }

    #[test]
    fn test_generate_create_table_validation() {
        // Empty table name
        let def = CreateTableDef::new("").column(ColumnDef::new("id", "INT"));
        assert!(generate_create_table_sql(&def, DatabaseFamily::Sqlite).is_err());

        // No columns
        let def = CreateTableDef::new("empty");
        assert!(generate_create_table_sql(&def, DatabaseFamily::Sqlite).is_err());

        // Duplicate column names
        let def = CreateTableDef::new("dup")
            .column(ColumnDef::new("name", "TEXT"))
            .column(ColumnDef::new("Name", "VARCHAR(50)"));
        assert!(generate_create_table_sql(&def, DatabaseFamily::Sqlite).is_err());
    }

    #[test]
    fn test_generate_create_table_sql_with_column_comments() {
        let def = CreateTableDef::new("users")
            .comment(Some("Table description".into()))
            .column(
                ColumnDef::new("id", "BIGINT")
                    .auto_increment(true)
                    .comment(Some("User primary key".into())),
            )
            .column(
                ColumnDef::new("email", "VARCHAR(255)")
                    .nullable(false)
                    .comment(Some("Login email".into())),
            )
            .column(ColumnDef::new("bio", "TEXT").comment(Some("User's profile bio".into())));

        // PostgreSQL: comments generated as COMMENT ON statements
        let pg_ddl = generate_create_table_sql(&def, DatabaseFamily::Postgres).unwrap();
        assert!(pg_ddl.contains("COMMENT ON TABLE \"users\" IS 'Table description';"));
        assert!(pg_ddl.contains("COMMENT ON COLUMN \"users\".\"id\" IS 'User primary key';"));
        assert!(pg_ddl.contains("COMMENT ON COLUMN \"users\".\"email\" IS 'Login email';"));
        assert!(pg_ddl.contains("COMMENT ON COLUMN \"users\".\"bio\" IS 'User''s profile bio';"));

        // MySQL: comments generated inline
        let mysql_ddl = generate_create_table_sql(&def, DatabaseFamily::MySql).unwrap();
        assert!(mysql_ddl.contains("COMMENT='Table description';"));
        assert!(
            mysql_ddl.contains("`id` BIGINT NOT NULL AUTO_INCREMENT COMMENT 'User primary key',")
        );
        assert!(mysql_ddl.contains("`email` VARCHAR(255) NOT NULL COMMENT 'Login email',"));
        assert!(mysql_ddl.contains("`bio` TEXT COMMENT 'User''s profile bio'"));

        // SQLite: standard columns without error
        let sqlite_ddl = generate_create_table_sql(&def, DatabaseFamily::Sqlite).unwrap();
        assert!(sqlite_ddl.contains("CREATE TABLE \"users\" ("));
    }

    #[test]
    fn test_parse_sql_column_list_and_expressions() {
        let raw = r#"id, "user,name", `first,last`, [created,at], updated_at DESC, "status" ASC"#;
        let cols = parse_sql_column_list(raw);
        assert_eq!(cols.len(), 6);
        assert_eq!(cols[0], "id");
        assert_eq!(cols[1], r#""user,name""#);
        assert_eq!(cols[2], "`first,last`");
        assert_eq!(cols[3], "[created,at]");
        assert_eq!(cols[4], "updated_at DESC");
        assert_eq!(cols[5], r#""status" ASC"#);

        // Matching
        assert!(column_matches_index_spec("id", "id"));
        assert!(column_matches_index_spec("ID", "id"));
        assert!(column_matches_index_spec("updated_at DESC", "updated_at"));
        assert!(column_matches_index_spec(r#""status" ASC"#, "status"));
        assert!(column_matches_index_spec(
            r#""CaseSensitive""#,
            "CaseSensitive"
        ));
        assert!(!column_matches_index_spec(
            r#""CaseSensitive""#,
            "casesensitive"
        ));

        // Formatting
        assert_eq!(
            format_index_column_expr("id", DatabaseFamily::Sqlite),
            "\"id\""
        );
        assert_eq!(
            format_index_column_expr("col DESC", DatabaseFamily::MySql),
            "`col` DESC"
        );
        assert_eq!(
            format_index_column_expr("\"col\" ASC", DatabaseFamily::Postgres),
            "\"col\" ASC"
        );
    }

    #[test]
    fn test_insert_into_auto_increment_vs_non_auto_increment_pk() {
        // Table like mysql.help_topic where help_topic_id is a non-auto-increment integer PK
        let cols = vec![
            ColumnInfo {
                name: "help_topic_id".into(),
                data_type: "int unsigned".into(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "name".into(),
                data_type: "char(64)".into(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];
        let grid_cols = vec!["help_topic_id".to_string(), "name".to_string()];

        // Case A: User provides explicit ID value for non-auto PK in MySQL
        let mut cs_explicit = GridChangeset::new();
        cs_explicit.add_inserted_row(
            vec![
                QueryValue::Int(101),
                QueryValue::String("JOIN Syntax".into()),
            ],
            crate::db::changeset::InsertAnchor::default(),
        );

        let plan_explicit = generate_review_plan(
            "help_topic",
            Some("mysql"),
            DatabaseFamily::MySql,
            &cols,
            &grid_cols,
            &[],
            &cs_explicit,
        );

        assert!(plan_explicit.warnings.is_empty());
        assert!(plan_explicit.full_script.contains("INSERT INTO `mysql`.`help_topic` (`help_topic_id`, `name`) VALUES (101, 'JOIN Syntax');"));

        // Case B: User left non-auto-increment NOT NULL PK as NULL - must NOT be omitted as auto, and triggers warning
        let mut cs_null = GridChangeset::new();
        cs_null.add_inserted_row(
            vec![QueryValue::Null, QueryValue::String("JOIN Syntax".into())],
            crate::db::changeset::InsertAnchor::default(),
        );

        let plan_null = generate_review_plan(
            "help_topic",
            Some("mysql"),
            DatabaseFamily::MySql,
            &cols,
            &grid_cols,
            &[],
            &cs_null,
        );

        assert!(!plan_null.warnings.is_empty());
        assert!(plan_null.warnings[0].contains("help_topic_id"));
        assert!(plan_null.warnings[0].contains("is NOT NULL and has no default value"));

        // Case C: True auto-increment in MySQL with <auto> or Null - correctly omitted from INSERT columns
        let auto_cols = vec![
            ColumnInfo {
                name: "id".into(),
                data_type: "int".into(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: true,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "title".into(),
                data_type: "varchar(100)".into(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];
        let auto_grid_cols = vec!["id".to_string(), "title".to_string()];
        let mut cs_auto = GridChangeset::new();
        cs_auto.add_inserted_row(
            vec![
                QueryValue::String("<auto>".into()),
                QueryValue::String("New Post".into()),
            ],
            crate::db::changeset::InsertAnchor::default(),
        );

        let plan_auto = generate_review_plan(
            "posts",
            None,
            DatabaseFamily::MySql,
            &auto_cols,
            &auto_grid_cols,
            &[],
            &cs_auto,
        );

        assert!(plan_auto.warnings.is_empty());
        assert!(
            plan_auto
                .full_script
                .contains("INSERT INTO `posts` (`title`) VALUES ('New Post');")
        );
    }

    #[test]
    fn test_split_sql_statements() {
        let sql = r#"
        -- First comment with a semicolon;
        CREATE TABLE "users" (
            id SERIAL PRIMARY KEY,
            name VARCHAR(50) NOT NULL,
            bio TEXT DEFAULT 'Hello; world!'
        );

        /* Multi-line comment
           containing semicolon; inside */
        COMMENT ON COLUMN "users"."bio" IS 'User''s bio; info';

        CREATE UNIQUE INDEX "uk_name" ON "users" ("name");
        "#;

        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 3);
        assert!(stmts[0].starts_with("-- First comment with a semicolon;\n        CREATE TABLE \"users\""));
        assert!(stmts[0].ends_with("DEFAULT 'Hello; world!'\n        )"));
        assert!(stmts[1].contains("COMMENT ON COLUMN \"users\".\"bio\" IS 'User''s bio; info'"));
        assert_eq!(stmts[2], "CREATE UNIQUE INDEX \"uk_name\" ON \"users\" (\"name\")");
    }

    #[test]
    fn test_split_sql_statements_pg_dollar_quotes() {
        let sql = r#"
        CREATE OR REPLACE FUNCTION test_func() RETURNS void AS $$
        BEGIN
            SELECT 1;
            INSERT INTO t VALUES (2);
        END;
        $$ LANGUAGE plpgsql;

        SELECT 42;
        "#;

        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("INSERT INTO t VALUES (2);"));
        assert_eq!(stmts[1], "SELECT 42");
    }

    #[test]
    fn test_dialect_data_types_and_presets() {
        let pg_types = dialect_data_types(DatabaseFamily::Postgres);
        assert!(pg_types.contains(&"TIMESTAMPTZ"));
        assert!(pg_types.contains(&"SERIAL"));
        assert!(!pg_types.contains(&"DATETIME")); // Postgres has no DATETIME

        let mysql_types = dialect_data_types(DatabaseFamily::MySql);
        assert!(mysql_types.contains(&"DATETIME"));
        assert!(mysql_types.contains(&"INT"));

        let sqlite_types = dialect_data_types(DatabaseFamily::Sqlite);
        assert!(sqlite_types.contains(&"INTEGER"));
        assert!(sqlite_types.contains(&"TEXT"));

        let pg_presets = dialect_presets(DatabaseFamily::Postgres);
        assert!(pg_presets.contains(&"TIMESTAMPTZ"));
    }

    #[test]
    fn test_truncate_sql_snippet_utf8_char_boundary() {
        let sql = r#"COMMENT ON COLUMN "public"."lato_report"."created_at" IS '创建时间';"#;
        // Byte 58 is where '创' begins; byte 60 falls inside '创' (bytes 58..61).
        // Ensure truncate_sql_snippet does not panic and truncates cleanly at char boundary.
        let snippet = truncate_sql_snippet(sql, 60);
        assert!(snippet.ends_with("..."));
        assert!(snippet.starts_with("COMMENT ON COLUMN"));
        assert!(snippet.contains("创建"));

        // When max_chars is larger than character length, it should not truncate
        let short = "SELECT 1;";
        assert_eq!(truncate_sql_snippet(short, 60), "SELECT 1;");
    }

    #[test]
    fn test_split_sql_statements_with_chinese_comments() {
        let sql = r#"CREATE TABLE "public"."lato_report" (
 "id" SERIAL PRIMARY KEY,
 "name" VARCHAR(20) NOT NULL,
 "title" VARCHAR(30) NOT NULL,
 "url" VARCHAR(64) NOT NULL,
 "created_at" TIMESTAMP NOT NULL,
 "updated_at" TIMESTAMP NOT NULL,
 "pub_time" TIMESTAMP NOT NULL
);

COMMENT ON COLUMN "public"."lato_report"."id" IS '自增id';
COMMENT ON COLUMN "public"."lato_report"."name" IS '名字';
COMMENT ON COLUMN "public"."lato_report"."title" IS '标题';
COMMENT ON COLUMN "public"."lato_report"."url" IS '链接';
COMMENT ON COLUMN "public"."lato_report"."created_at" IS '创建时间';
COMMENT ON COLUMN "public"."lato_report"."updated_at" IS '修改时间';
COMMENT ON COLUMN "public"."lato_report"."pub_time" IS '发表时间';

CREATE UNIQUE INDEX "uk_name_title" ON "public"."lato_report" ("name", "title");"#;

        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 9);
        for stmt in &stmts {
            let snippet = truncate_sql_snippet(stmt, 60);
            assert!(!snippet.is_empty());
        }
    }

    #[test]
    fn test_parse_create_table_mysql() {
        let sql = r#"
        CREATE TABLE `shop_db`.`products` (
            `id` INT NOT NULL AUTO_INCREMENT,
            `title` VARCHAR(255) NOT NULL COMMENT 'Item title',
            `price` DECIMAL(10, 2) DEFAULT 0.00,
            `status` VARCHAR(20) DEFAULT 'draft',
            PRIMARY KEY (`id`),
            UNIQUE KEY `uk_title` (`title`),
            KEY `idx_status` (`status`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='Catalog products';
        "#;

        let def = parse_create_table_sql(sql, DatabaseFamily::MySql).unwrap();
        assert_eq!(def.table_name, "products");
        assert_eq!(def.schema.as_deref(), Some("shop_db"));
        assert_eq!(def.comment.as_deref(), Some("Catalog products"));
        assert_eq!(def.columns.len(), 4);

        // id
        assert_eq!(def.columns[0].name, "id");
        assert_eq!(def.columns[0].data_type, "INT");
        assert!(def.columns[0].is_primary_key);
        assert!(def.columns[0].is_auto_increment);
        assert!(!def.columns[0].is_nullable);

        // title
        assert_eq!(def.columns[1].name, "title");
        assert_eq!(def.columns[1].data_type, "VARCHAR(255)");
        assert_eq!(def.columns[1].comment.as_deref(), Some("Item title"));
        assert!(!def.columns[1].is_nullable);

        // price (verifying DECIMAL(10, 2) preserved correctly)
        assert_eq!(def.columns[2].name, "price");
        assert_eq!(def.columns[2].data_type, "DECIMAL(10, 2)");
        assert_eq!(def.columns[2].default_value.as_deref(), Some("0.00"));

        // indexes
        assert_eq!(def.indexes.len(), 2);
        assert_eq!(def.indexes[0].name, "uk_title");
        assert_eq!(def.indexes[0].index_type, TableIndexType::Unique);
        assert_eq!(def.indexes[0].columns, vec!["`title`"]);
    }

    #[test]
    fn test_parse_create_table_postgres_with_comments() {
        let sql = r#"
        CREATE TABLE "public"."users" (
            "id" SERIAL PRIMARY KEY,
            "username" VARCHAR(50) NOT NULL,
            "bio" TEXT,
            "created_at" TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
        );

        COMMENT ON TABLE "public"."users" IS 'User account records';
        COMMENT ON COLUMN "public"."users"."id" IS 'Auto-increment primary key';
        COMMENT ON COLUMN "public"."users"."username" IS 'Unique login name';

        CREATE UNIQUE INDEX "uk_username" ON "public"."users" ("username");
        "#;

        let def = parse_create_table_sql(sql, DatabaseFamily::Postgres).unwrap();
        assert_eq!(def.table_name, "users");
        assert_eq!(def.schema.as_deref(), Some("public"));
        assert_eq!(def.comment.as_deref(), Some("User account records"));
        assert_eq!(def.columns.len(), 4);

        assert_eq!(def.columns[0].name, "id");
        assert!(def.columns[0].is_primary_key);
        assert!(def.columns[0].is_auto_increment);
        assert_eq!(def.columns[0].comment.as_deref(), Some("Auto-increment primary key"));

        assert_eq!(def.columns[1].name, "username");
        assert_eq!(def.columns[1].comment.as_deref(), Some("Unique login name"));

        assert_eq!(def.columns[3].name, "created_at");
        assert_eq!(def.columns[3].data_type, "TIMESTAMPTZ");
        assert_eq!(def.columns[3].default_value.as_deref(), Some("CURRENT_TIMESTAMP"));

        assert_eq!(def.indexes.len(), 1);
        assert_eq!(def.indexes[0].name, "uk_username");
        assert_eq!(def.indexes[0].index_type, TableIndexType::Unique);
    }

    #[test]
    fn test_parse_create_table_sqlite() {
        let sql = r#"
        CREATE TABLE "items" (
            "id" INTEGER PRIMARY KEY AUTOINCREMENT,
            "name" TEXT NOT NULL,
            "count" INTEGER DEFAULT 1
        );
        "#;

        let def = parse_create_table_sql(sql, DatabaseFamily::Sqlite).unwrap();
        assert_eq!(def.table_name, "items");
        assert_eq!(def.columns.len(), 3);
        assert!(def.columns[0].is_primary_key);
        assert!(def.columns[0].is_auto_increment);
        assert_eq!(def.columns[1].name, "name");
        assert!(!def.columns[1].is_nullable);
        assert_eq!(def.columns[2].default_value.as_deref(), Some("1"));
    }

    #[test]
    fn test_parse_create_table_invalid_inputs() {
        assert!(parse_create_table_sql("", DatabaseFamily::Sqlite).is_err());
        assert!(parse_create_table_sql("SELECT * FROM users;", DatabaseFamily::Sqlite).is_err());
        assert!(parse_create_table_sql("CREATE TABLE missing_parenthesis", DatabaseFamily::Sqlite).is_err());
    }

    #[test]
    fn test_build_insert_template_with_auto_increment() {
        let cols = sample_columns(); // id (auto-inc), name, note
        let tbl = TableInfo {
            name: "users".to_string(),
            schema: Some("public".to_string()),
            table_type: "BASE TABLE".to_string(),
            comment: None,
            row_count_estimate: None,
        };

        // PostgreSQL: omits auto-increment id and quotes columns/tables
        let pg_insert = build_insert_template(&tbl, &cols, DatabaseFamily::Postgres);
        assert!(pg_insert.contains("INSERT INTO \"public\".\"users\""));
        assert!(pg_insert.contains("(\"name\", \"note\")"));
        assert!(pg_insert.contains("VALUES (?, ?)"));

        // MySQL
        let mysql_insert = build_insert_template(&tbl, &cols, DatabaseFamily::MySql);
        assert!(mysql_insert.contains("INSERT INTO `public`.`users`"));
        assert!(mysql_insert.contains("(`name`, `note`)"));
        assert!(mysql_insert.contains("VALUES (?, ?)"));
    }

    #[test]
    fn test_generate_alter_table_plan_mysql() {
        let orig_cols = vec![
            ColumnInfo {
                name: "id".into(),
                data_type: "INT".into(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: true,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "title".into(),
                data_type: "VARCHAR(100)".into(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "obsolete".into(),
                data_type: "TEXT".into(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];

        // Target:
        // 1. Keep id
        // 2. Modify title -> VARCHAR(255) nullable with comment
        // 3. Drop obsolete (omitted)
        // 4. Add new column `status`
        let target_cols = vec![
            AlterColumnTarget::new(
                Some("id".into()),
                ColumnDef::new("id", "INT").primary_key(true).auto_increment(true),
            ),
            AlterColumnTarget::new(
                Some("title".into()),
                ColumnDef::new("title", "VARCHAR(255)")
                    .nullable(true)
                    .comment(Some("Updated title".into())),
            ),
            AlterColumnTarget::new(
                None,
                ColumnDef::new("status", "VARCHAR(20)")
                    .nullable(false)
                    .default_value(Some("'active'".into())),
            ),
        ];

        let plan = generate_alter_table_plan(
            "articles",
            Some("cms"),
            DatabaseFamily::MySql,
            &orig_cols,
            &target_cols,
        );

        assert_eq!(plan.alterations.len(), 3);
        assert!(plan.statements.iter().any(|s| s.contains("DROP COLUMN `obsolete`")));
        assert!(plan.statements.iter().any(|s| s.contains("MODIFY COLUMN `title` VARCHAR(255) COMMENT 'Updated title'")));
        assert!(plan.statements.iter().any(|s| s.contains("ADD COLUMN `status` VARCHAR(20) NOT NULL DEFAULT 'active' AFTER `title`")));
        assert!(plan.full_script.starts_with("START TRANSACTION;"));
        assert!(plan.full_script.ends_with("COMMIT;"));

        let review_plan = plan.to_review_plan();
        assert_eq!(review_plan.inserts_count, 1); // 1 add
        assert_eq!(review_plan.updates_count, 1); // 1 modify
        assert_eq!(review_plan.deletes_count, 1); // 1 drop
    }

    #[test]
    fn test_generate_alter_table_plan_postgres() {
        let orig_cols = vec![
            ColumnInfo {
                name: "id".into(),
                data_type: "BIGSERIAL".into(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: true,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "nickname".into(),
                data_type: "VARCHAR(50)".into(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];

        // Rename nickname -> display_name, change type to VARCHAR(100), make NOT NULL
        let target_cols = vec![
            AlterColumnTarget::new(
                Some("id".into()),
                ColumnDef::new("id", "BIGSERIAL").primary_key(true).auto_increment(true),
            ),
            AlterColumnTarget::new(
                Some("nickname".into()),
                ColumnDef::new("display_name", "VARCHAR(100)").nullable(false),
            ),
        ];

        let plan = generate_alter_table_plan(
            "members",
            Some("public"),
            DatabaseFamily::Postgres,
            &orig_cols,
            &target_cols,
        );

        assert!(plan.statements.iter().any(|s| s.contains("RENAME COLUMN \"nickname\" TO \"display_name\"")));
        assert!(plan.statements.iter().any(|s| s.contains("ALTER COLUMN \"display_name\" TYPE VARCHAR(100)")));
        assert!(plan.full_script.starts_with("BEGIN;"));
        assert!(plan.full_script.ends_with("COMMIT;"));
    }

    #[test]
    fn test_generate_alter_table_plan_sqlite() {
        let orig_cols = vec![
            ColumnInfo {
                name: "id".into(),
                data_type: "INTEGER".into(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: true,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "score".into(),
                data_type: "INTEGER".into(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];

        // Case 1: Simple ADD COLUMN in SQLite -> native ALTER TABLE ADD COLUMN
        let add_only_target = vec![
            AlterColumnTarget::new(
                Some("id".into()),
                ColumnDef::new("id", "INTEGER").primary_key(true).auto_increment(true),
            ),
            AlterColumnTarget::new(
                Some("score".into()),
                ColumnDef::new("score", "INTEGER").nullable(true),
            ),
            AlterColumnTarget::new(
                None,
                ColumnDef::new("extra", "TEXT").default_value(Some("''".into())),
            ),
        ];

        let plan_add = generate_alter_table_plan(
            "games",
            None,
            DatabaseFamily::Sqlite,
            &orig_cols,
            &add_only_target,
        );
        assert!(plan_add.statements.iter().any(|s| s.contains("ALTER TABLE \"games\" ADD COLUMN \"extra\" TEXT DEFAULT ''")));

        // Case 2: Dropping or modifying columns in SQLite -> safe table recreation migration
        let modify_target = vec![
            AlterColumnTarget::new(
                Some("id".into()),
                ColumnDef::new("id", "INTEGER").primary_key(true).auto_increment(true),
            ),
            // Dropped score, added points
            AlterColumnTarget::new(
                None,
                ColumnDef::new("points", "REAL").default_value(Some("0.0".into())),
            ),
        ];

        let plan_recreate = generate_alter_table_plan(
            "games",
            None,
            DatabaseFamily::Sqlite,
            &orig_cols,
            &modify_target,
        );

        assert!(!plan_recreate.warnings.is_empty());
        assert!(plan_recreate.warnings[0].contains("recreating table 'games'"));
        assert!(plan_recreate.statements.iter().any(|s| s.contains("PRAGMA foreign_keys = OFF;")));
        assert!(plan_recreate.statements.iter().any(|s| s.contains("CREATE TABLE \"games_new_migration\"")));
        assert!(plan_recreate.statements.iter().any(|s| s.contains("INSERT INTO \"games_new_migration\" (\"id\")")));
        assert!(plan_recreate.statements.iter().any(|s| s.contains("DROP TABLE \"games\";")));
        assert!(plan_recreate.statements.iter().any(|s| s.contains("ALTER TABLE \"games_new_migration\" RENAME TO \"games\";")));
        assert!(plan_recreate.statements.iter().any(|s| s.contains("PRAGMA foreign_keys = ON;")));
    }
}

