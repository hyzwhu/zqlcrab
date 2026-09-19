//! SQL statement generator for atomic tabular updates and deletions across database dialects.

use crate::db::changeset::GridChangeset;
use crate::db::types::{quote_ident, ColumnInfo, DatabaseFamily, QueryValue};
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
            format!("{}.{}", quote_ident(s, family), quote_ident(table_name, family))
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
            let col_meta = columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name));

            // Determine whether to omit an auto-increment or serial column when value is Null or <auto>
            let is_auto = col_meta.map(|c| {
                c.is_auto_increment
                    || c.data_type.to_lowercase().contains("serial")
                    || (c.is_primary_key && (c.data_type.to_lowercase().contains("int") || family == DatabaseFamily::Sqlite))
            }).unwrap_or(false);

            let is_auto_placeholder = match val {
                QueryValue::Null => is_auto,
                QueryValue::String(s) if s.trim().eq_ignore_ascii_case("<auto>") => true,
                _ => false,
            };

            let has_default_and_not_nullable = col_meta.map(|c| {
                !c.is_nullable && (c.default_value.is_some() || c.is_auto_increment)
            }).unwrap_or(false);

            let is_default_placeholder = match val {
                QueryValue::Null => has_default_and_not_nullable,
                QueryValue::String(s) if s.trim().eq_ignore_ascii_case("<default>") => true,
                _ => false,
            };

            if is_auto_placeholder || is_default_placeholder {
                continue;
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
        row_updates
            .entry(r_idx)
            .or_default()
            .push((c_idx, edit.column_name.clone(), edit.new_value.clone()));
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
        let where_clause = build_row_where_clause(
            family,
            columns,
            grid_columns,
            orig_row,
            &pk_names,
        );

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

        statements.push(format!("DELETE FROM {qualified_table}\nWHERE {where_clause};"));
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
            let col_idx = grid_columns.iter().position(|c| c.eq_ignore_ascii_case(pk))
                .or_else(|| columns.iter().position(|c| c.name.eq_ignore_ascii_case(pk)));

            if let Some(idx) = col_idx {
                if let Some(val) = row_values.get(idx) {
                    let col_quoted = quote_ident(pk, family);
                    if val.is_null() {
                        clauses.push(format!("{col_quoted} IS NULL"));
                    } else {
                        clauses.push(format!("{col_quoted} = {}", format_query_value(val, family)));
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
                    clauses.push(format!("{col_quoted} = {}", format_query_value(val, family)));
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
            DatabaseFamily::MySql | DatabaseFamily::Sqlite => if *b { "1" } else { "0" }.to_string(),
        },
        QueryValue::Int(i) => i.to_string(),
        QueryValue::Float(f) => {
            let s = format!("{f}");
            if s.contains('.') {
                s
            } else {
                format!("{f}.0")
            }
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
            format!(
                "START TRANSACTION;\n\n{joined_stmts}\n\nCOMMIT;"
            )
        }
        DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
            format!(
                "BEGIN;\n\n{joined_stmts}\n\nCOMMIT;"
            )
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
    let trimmed = raw_target.trim_matches(|c| c == ';' || c == ',' || c == ')' || c == '(').trim();
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
            Some((if schema.is_empty() { None } else { Some(schema) }, tbl))
        }
    } else {
        let schema = strip_identifier_quotes(parts[parts.len() - 2]);
        let tbl = strip_identifier_quotes(parts[parts.len() - 1]);
        if tbl.is_empty() {
            None
        } else {
            Some((if schema.is_empty() { None } else { Some(schema) }, tbl))
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
    let is_quoted = trimmed.starts_with('"') || trimmed.starts_with('`') || trimmed.starts_with('[');

    if is_quoted {
        base == col_name
    } else {
        base.eq_ignore_ascii_case(col_name)
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
            format!("{}.{}", quote_ident(schema_trimmed, family), quote_ident(table_name, family))
        } else {
            quote_ident(table_name, family)
        }
    } else {
        quote_ident(table_name, family)
    };

    let pks: Vec<&ColumnDef> = valid_cols.iter().filter(|c| c.is_primary_key).copied().collect();

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
                    col_clauses.push(format!("    {quoted_col} INTEGER PRIMARY KEY AUTOINCREMENT"));
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
                    let sanitized: Vec<String> = valid_idx_cols.iter().map(|c| extract_base_column_name(c)).collect();
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
                    let sanitized: Vec<String> = valid_idx_cols.iter().map(|c| extract_base_column_name(c)).collect();
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
                    let sanitized: Vec<String> = valid_idx_cols.iter().map(|c| extract_base_column_name(c)).collect();
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
            vec![QueryValue::Int(1), QueryValue::String("Alice".into()), QueryValue::Null],
            vec![QueryValue::Int(2), QueryValue::String("Bob".into()), QueryValue::String("Hello".into())],
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
        let orig_rows = vec![
            vec![QueryValue::Null, QueryValue::Int(1)],
        ];

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
        let orig_rows = vec![
            vec![QueryValue::Int(10), QueryValue::String("O'Reilly".into()), QueryValue::Null],
        ];

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
        let grid_cols = vec!["id".to_string(), "name".to_string(), "email".to_string(), "age".to_string(), "active".to_string()];
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
        assert!(script.contains("INSERT INTO `shop_db`.`products` (`name`) VALUES ('Frank\\'s \"Gadgets\"');"));
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
        let res = extract_table_from_sql("SELECT id, name FROM \"public\".\"users\" WHERE active = true;");
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
            .column(ColumnDef::new("created_at", "DATETIME").default_value(Some("CURRENT_TIMESTAMP".into())));

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
            .column(ColumnDef::new("id", "BIGINT").auto_increment(true).comment(Some("Unique ID".into())))
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
            .column(ColumnDef::new("title", "VARCHAR(255)").nullable(false).comment(Some("Item name".into())))
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
            .index(TableIndexDef::new("", vec!["tenant_id".into(), "status".into()])); // Auto name idx_accounts_tenant_id_status

        // SQLite
        let sqlite_ddl = generate_create_table_sql(&def, DatabaseFamily::Sqlite).unwrap();
        assert!(sqlite_ddl.contains("CREATE TABLE \"core\".\"accounts\" ("));
        assert!(sqlite_ddl.contains("CREATE UNIQUE INDEX \"uk_accounts_email\" ON \"core\".\"accounts\" (\"email\");"));
        assert!(sqlite_ddl.contains("CREATE INDEX \"idx_accounts_tenant_id_status\" ON \"core\".\"accounts\" (\"tenant_id\", \"status\");"));

        // PostgreSQL
        let pg_ddl = generate_create_table_sql(&def, DatabaseFamily::Postgres).unwrap();
        assert!(pg_ddl.contains("CREATE TABLE \"core\".\"accounts\" ("));
        assert!(pg_ddl.contains("CREATE UNIQUE INDEX \"uk_accounts_email\" ON \"core\".\"accounts\" (\"email\");"));
        assert!(pg_ddl.contains("CREATE INDEX \"idx_accounts_tenant_id_status\" ON \"core\".\"accounts\" (\"tenant_id\", \"status\");"));

        // MySQL
        let mysql_ddl = generate_create_table_sql(&def, DatabaseFamily::MySql).unwrap();
        assert!(mysql_ddl.contains("CREATE TABLE `core`.`accounts` ("));
        assert!(mysql_ddl.contains("    UNIQUE KEY `uk_accounts_email` (`email`),"));
        assert!(mysql_ddl.contains("    KEY `idx_accounts_tenant_id_status` (`tenant_id`, `status`)"));
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
            .column(ColumnDef::new("id", "BIGINT").auto_increment(true).comment(Some("User primary key".into())))
            .column(ColumnDef::new("email", "VARCHAR(255)").nullable(false).comment(Some("Login email".into())))
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
        assert!(mysql_ddl.contains("`id` BIGINT NOT NULL AUTO_INCREMENT COMMENT 'User primary key',"));
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
        assert!(column_matches_index_spec(r#""CaseSensitive""#, "CaseSensitive"));
        assert!(!column_matches_index_spec(r#""CaseSensitive""#, "casesensitive"));

        // Formatting
        assert_eq!(format_index_column_expr("id", DatabaseFamily::Sqlite), "\"id\"");
        assert_eq!(format_index_column_expr("col DESC", DatabaseFamily::MySql), "`col` DESC");
        assert_eq!(format_index_column_expr("\"col\" ASC", DatabaseFamily::Postgres), "\"col\" ASC");
    }
}
