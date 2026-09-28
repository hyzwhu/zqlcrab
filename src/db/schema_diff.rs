//! Table schema discrepancy detection and multi-dialect migration script generator.
//!
//! Compares column specifications (data type, nullability, defaults, primary keys)
//! and indexes between two tables, producing side-by-side diff summaries and dialect-accurate
//! DDL migration scripts for PostgreSQL, MySQL, and SQLite.

use crate::db::types::{ColumnInfo, DatabaseFamily, IndexInfo, quote_ident};
use serde::{Deserialize, Serialize};

/// Direction of migration synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MigrationDirection {
    /// Alter Target table so that its structure matches the Source table.
    #[default]
    SourceToTarget,
    /// Alter Source table so that its structure matches the Target table.
    TargetToSource,
}

/// Comparison status for an individual column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnDiffStatus {
    /// Column exists in Source but is missing in Target.
    Added,
    /// Column exists in Target but is missing in Source.
    Dropped,
    /// Column exists in both tables but has attribute discrepancies.
    Modified {
        type_diff: bool,
        null_diff: bool,
        default_diff: bool,
        pk_diff: bool,
    },
    /// Column is identical across both tables.
    Unchanged,
}

impl ColumnDiffStatus {
    pub fn is_different(&self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    pub fn badge_label(&self) -> &'static str {
        match self {
            Self::Added => "ADD",
            Self::Dropped => "DROP",
            Self::Modified { .. } => "MODIFY",
            Self::Unchanged => "IDENTICAL",
        }
    }
}

/// Detailed column discrepancy record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnDiff {
    pub name: String,
    pub status: ColumnDiffStatus,
    pub source: Option<ColumnInfo>,
    pub target: Option<ColumnInfo>,
    pub details: Vec<String>,
}

/// Comparison status for a table index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexDiffStatus {
    /// Index exists in Source but missing in Target.
    Added,
    /// Index exists in Target but missing in Source.
    Dropped,
    /// Index exists in both but columns or uniqueness differ.
    Modified,
    /// Index is identical in both.
    Unchanged,
}

impl IndexDiffStatus {
    pub fn is_different(&self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    pub fn badge_label(&self) -> &'static str {
        match self {
            Self::Added => "ADD",
            Self::Dropped => "DROP",
            Self::Modified => "MODIFY",
            Self::Unchanged => "IDENTICAL",
        }
    }
}

/// Detailed index discrepancy record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDiff {
    pub name: String,
    pub status: IndexDiffStatus,
    pub source: Option<IndexInfo>,
    pub target: Option<IndexInfo>,
    pub details: Vec<String>,
}

/// Summary metrics of structural differences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDiffSummary {
    pub added_columns: usize,
    pub modified_columns: usize,
    pub dropped_columns: usize,
    pub unchanged_columns: usize,
    pub added_indexes: usize,
    pub modified_indexes: usize,
    pub dropped_indexes: usize,
    pub unchanged_indexes: usize,
}

impl SchemaDiffSummary {
    pub fn total_differences(&self) -> usize {
        self.added_columns
            + self.modified_columns
            + self.dropped_columns
            + self.added_indexes
            + self.modified_indexes
            + self.dropped_indexes
    }

    pub fn has_differences(&self) -> bool {
        self.total_differences() > 0
    }
}

/// Complete comparison report between two tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDiffReport {
    pub source_table: String,
    pub source_schema: Option<String>,
    pub target_table: String,
    pub target_schema: Option<String>,
    pub family: DatabaseFamily,
    pub column_diffs: Vec<ColumnDiff>,
    pub index_diffs: Vec<IndexDiff>,
    pub summary: SchemaDiffSummary,
}

impl Default for SchemaDiffReport {
    fn default() -> Self {
        Self {
            source_table: String::new(),
            source_schema: None,
            target_table: String::new(),
            target_schema: None,
            family: DatabaseFamily::Sqlite,
            column_diffs: Vec::new(),
            index_diffs: Vec::new(),
            summary: SchemaDiffSummary::default(),
        }
    }
}

/// Migration generation configuration options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDiffOptions {
    pub direction: MigrationDirection,
    /// Whether to generate destructive DROP statements. If false, DROP queries are commented out.
    pub include_drops: bool,
    /// Whether to wrap the generated statements in a transaction (BEGIN / COMMIT).
    pub wrap_transaction: bool,
}

impl Default for SchemaDiffOptions {
    fn default() -> Self {
        Self {
            direction: MigrationDirection::SourceToTarget,
            include_drops: false,
            wrap_transaction: true,
        }
    }
}

/// Result of migration script generation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationScript {
    pub statements: Vec<String>,
    pub full_script: String,
    pub warnings: Vec<String>,
}

/// Compares two tables and generates a side-by-side discrepancy report.
pub fn compare_tables(
    source_table: &str,
    source_schema: Option<&str>,
    source_cols: &[ColumnInfo],
    source_indexes: &[IndexInfo],
    target_table: &str,
    target_schema: Option<&str>,
    target_cols: &[ColumnInfo],
    target_indexes: &[IndexInfo],
    family: DatabaseFamily,
) -> SchemaDiffReport {
    let mut column_diffs = Vec::new();
    let mut summary = SchemaDiffSummary::default();

    // 1. Process all source columns (detect Added, Modified, or Unchanged)
    for src_col in source_cols {
        let tgt_match = target_cols
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&src_col.name));

        match tgt_match {
            None => {
                summary.added_columns += 1;
                column_diffs.push(ColumnDiff {
                    name: src_col.name.clone(),
                    status: ColumnDiffStatus::Added,
                    source: Some(src_col.clone()),
                    target: None,
                    details: vec![format!("Column does not exist in target table")],
                });
            }
            Some(tgt_col) => {
                let type_diff = !types_match(&src_col.data_type, &tgt_col.data_type, family);
                let null_diff = src_col.is_nullable != tgt_col.is_nullable;
                let default_diff = !defaults_match(
                    src_col.default_value.as_deref(),
                    tgt_col.default_value.as_deref(),
                );
                let pk_diff = src_col.is_primary_key != tgt_col.is_primary_key;

                let mut details = Vec::new();
                if type_diff {
                    details.push(format!(
                        "Type: {} -> {}",
                        tgt_col.data_type, src_col.data_type
                    ));
                }
                if null_diff {
                    details.push(format!(
                        "Nullable: {} -> {}",
                        tgt_col.is_nullable, src_col.is_nullable
                    ));
                }
                if default_diff {
                    let d_src = src_col.default_value.as_deref().unwrap_or("NULL");
                    let d_tgt = tgt_col.default_value.as_deref().unwrap_or("NULL");
                    details.push(format!("Default: {} -> {}", d_tgt, d_src));
                }
                if pk_diff {
                    details.push(format!(
                        "PK: {} -> {}",
                        tgt_col.is_primary_key, src_col.is_primary_key
                    ));
                }

                if type_diff || null_diff || default_diff || pk_diff {
                    summary.modified_columns += 1;
                    column_diffs.push(ColumnDiff {
                        name: src_col.name.clone(),
                        status: ColumnDiffStatus::Modified {
                            type_diff,
                            null_diff,
                            default_diff,
                            pk_diff,
                        },
                        source: Some(src_col.clone()),
                        target: Some(tgt_col.clone()),
                        details,
                    });
                } else {
                    summary.unchanged_columns += 1;
                    column_diffs.push(ColumnDiff {
                        name: src_col.name.clone(),
                        status: ColumnDiffStatus::Unchanged,
                        source: Some(src_col.clone()),
                        target: Some(tgt_col.clone()),
                        details: vec!["Definitions match".to_string()],
                    });
                }
            }
        }
    }

    // 2. Process Target columns that do not exist in Source (Dropped)
    for tgt_col in target_cols {
        let exists_in_src = source_cols
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&tgt_col.name));

        if !exists_in_src {
            summary.dropped_columns += 1;
            column_diffs.push(ColumnDiff {
                name: tgt_col.name.clone(),
                status: ColumnDiffStatus::Dropped,
                source: None,
                target: Some(tgt_col.clone()),
                details: vec!["Column exists in target but missing in source".to_string()],
            });
        }
    }

    // 3. Process Indexes
    let mut index_diffs = Vec::new();
    for src_idx in source_indexes {
        // Skip primary key pseudo-indexes if handled by PK column flags
        if src_idx.is_primary {
            continue;
        }

        let tgt_match = target_indexes.iter().find(|i| {
            i.name.eq_ignore_ascii_case(&src_idx.name) || index_columns_match(i, src_idx)
        });

        match tgt_match {
            None => {
                summary.added_indexes += 1;
                index_diffs.push(IndexDiff {
                    name: src_idx.name.clone(),
                    status: IndexDiffStatus::Added,
                    source: Some(src_idx.clone()),
                    target: None,
                    details: vec![format!("Columns: ({})", src_idx.columns.join(", "))],
                });
            }
            Some(tgt_idx) => {
                let unique_diff = src_idx.is_unique != tgt_idx.is_unique;
                let cols_diff = !index_columns_match(src_idx, tgt_idx);

                if unique_diff || cols_diff {
                    let mut details = Vec::new();
                    if unique_diff {
                        details.push(format!(
                            "Unique: {} -> {}",
                            tgt_idx.is_unique, src_idx.is_unique
                        ));
                    }
                    if cols_diff {
                        details.push(format!(
                            "Cols: ({}) -> ({})",
                            tgt_idx.columns.join(", "),
                            src_idx.columns.join(", ")
                        ));
                    }
                    summary.modified_indexes += 1;
                    index_diffs.push(IndexDiff {
                        name: src_idx.name.clone(),
                        status: IndexDiffStatus::Modified,
                        source: Some(src_idx.clone()),
                        target: Some(tgt_idx.clone()),
                        details,
                    });
                } else {
                    summary.unchanged_indexes += 1;
                    index_diffs.push(IndexDiff {
                        name: src_idx.name.clone(),
                        status: IndexDiffStatus::Unchanged,
                        source: Some(src_idx.clone()),
                        target: Some(tgt_idx.clone()),
                        details: vec!["Indexes match".to_string()],
                    });
                }
            }
        }
    }

    for tgt_idx in target_indexes {
        if tgt_idx.is_primary {
            continue;
        }
        let exists_in_src = source_indexes
            .iter()
            .any(|i| i.name.eq_ignore_ascii_case(&tgt_idx.name) || index_columns_match(i, tgt_idx));

        if !exists_in_src {
            summary.dropped_indexes += 1;
            index_diffs.push(IndexDiff {
                name: tgt_idx.name.clone(),
                status: IndexDiffStatus::Dropped,
                source: None,
                target: Some(tgt_idx.clone()),
                details: vec![format!("Columns: ({})", tgt_idx.columns.join(", "))],
            });
        }
    }

    SchemaDiffReport {
        source_table: source_table.to_string(),
        source_schema: source_schema.map(ToString::to_string),
        target_table: target_table.to_string(),
        target_schema: target_schema.map(ToString::to_string),
        family,
        column_diffs,
        index_diffs,
        summary,
    }
}

/// Generates dialect-specific incremental DDL migration scripts based on the diff report.
pub fn generate_migration_ddl(
    report: &SchemaDiffReport,
    options: &SchemaDiffOptions,
) -> MigrationScript {
    let mut statements = Vec::new();
    let mut warnings = Vec::new();

    let target_table_ident = format_table_identifier(
        &report.target_table,
        report.target_schema.as_deref(),
        report.family,
    );

    let is_source_to_target = options.direction == MigrationDirection::SourceToTarget;

    // Check for SQLite limitations early
    let is_sqlite = report.family == DatabaseFamily::Sqlite;
    let mut sqlite_needs_rebuild = false;

    // 1. Column Migrations
    for diff in &report.column_diffs {
        match &diff.status {
            ColumnDiffStatus::Added => {
                if is_source_to_target {
                    if let Some(col) = &diff.source {
                        let add_sql = format!(
                            "ALTER TABLE {} ADD COLUMN {};",
                            target_table_ident,
                            render_column_definition(col, report.family)
                        );
                        statements.push(add_sql);
                    }
                } else {
                    // Reverse: Target has to drop what source added
                    if options.include_drops {
                        let drop_sql = format!(
                            "ALTER TABLE {} DROP COLUMN {};",
                            target_table_ident,
                            quote_ident(&diff.name, report.family)
                        );
                        statements.push(drop_sql);
                    } else {
                        statements.push(format!(
                            "-- [SAFE MODE SKIPPED] ALTER TABLE {} DROP COLUMN {};",
                            target_table_ident,
                            quote_ident(&diff.name, report.family)
                        ));
                    }
                }
            }
            ColumnDiffStatus::Dropped => {
                if is_source_to_target {
                    // Target has column that source lacks -> DROP in Target
                    if options.include_drops {
                        let drop_sql = format!(
                            "ALTER TABLE {} DROP COLUMN {};",
                            target_table_ident,
                            quote_ident(&diff.name, report.family)
                        );
                        statements.push(drop_sql);
                    } else {
                        statements.push(format!(
                            "-- [SAFE MODE SKIPPED] ALTER TABLE {} DROP COLUMN {};",
                            target_table_ident,
                            quote_ident(&diff.name, report.family)
                        ));
                        warnings.push(format!(
                            "Column '{}' was not dropped because Safe Mode is enabled.",
                            diff.name
                        ));
                    }
                } else {
                    // Reverse: Target must ADD this column
                    if let Some(col) = &diff.target {
                        let add_sql = format!(
                            "ALTER TABLE {} ADD COLUMN {};",
                            target_table_ident,
                            render_column_definition(col, report.family)
                        );
                        statements.push(add_sql);
                    }
                }
            }
            ColumnDiffStatus::Modified {
                type_diff,
                null_diff,
                default_diff,
                pk_diff,
            } => {
                let target_col = if is_source_to_target {
                    diff.source.as_ref()
                } else {
                    diff.target.as_ref()
                };

                if let Some(col) = target_col {
                    if is_sqlite {
                        sqlite_needs_rebuild = true;
                        warnings.push(format!(
                            "SQLite does not support direct in-place column modification for '{}'. Safe table migration is advised.",
                            col.name
                        ));
                    } else {
                        match report.family {
                            DatabaseFamily::Postgres => {
                                let col_name_q = quote_ident(&col.name, report.family);
                                if *type_diff {
                                    statements.push(format!(
                                        "ALTER TABLE {} ALTER COLUMN {} TYPE {};",
                                        target_table_ident, col_name_q, col.data_type
                                    ));
                                }
                                if *null_diff {
                                    let null_action = if col.is_nullable {
                                        "DROP NOT NULL"
                                    } else {
                                        "SET NOT NULL"
                                    };
                                    statements.push(format!(
                                        "ALTER TABLE {} ALTER COLUMN {} {};",
                                        target_table_ident, col_name_q, null_action
                                    ));
                                }
                                if *default_diff {
                                    if let Some(def) = &col.default_value {
                                        statements.push(format!(
                                            "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {};",
                                            target_table_ident, col_name_q, def
                                        ));
                                    } else {
                                        statements.push(format!(
                                            "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT;",
                                            target_table_ident, col_name_q
                                        ));
                                    }
                                }
                                if *pk_diff {
                                    warnings.push(format!(
                                        "Primary key alteration for column '{}' requires updating table constraints.",
                                        col.name
                                    ));
                                }
                            }
                            DatabaseFamily::MySql => {
                                statements.push(format!(
                                    "ALTER TABLE {} MODIFY COLUMN {};",
                                    target_table_ident,
                                    render_column_definition(col, report.family)
                                ));
                            }
                            DatabaseFamily::Sqlite => {}
                        }
                    }
                }
            }
            ColumnDiffStatus::Unchanged => {}
        }
    }

    // 2. Index Migrations
    for diff in &report.index_diffs {
        match &diff.status {
            IndexDiffStatus::Added => {
                if is_source_to_target {
                    if let Some(idx) = &diff.source {
                        statements.push(render_create_index(
                            idx,
                            &target_table_ident,
                            report.family,
                        ));
                    }
                } else if options.include_drops {
                    statements.push(render_drop_index(
                        &diff.name,
                        &target_table_ident,
                        report.family,
                    ));
                } else {
                    statements.push(format!(
                        "-- [SAFE MODE SKIPPED] {}",
                        render_drop_index(&diff.name, &target_table_ident, report.family)
                    ));
                }
            }
            IndexDiffStatus::Dropped => {
                if is_source_to_target {
                    if options.include_drops {
                        statements.push(render_drop_index(
                            &diff.name,
                            &target_table_ident,
                            report.family,
                        ));
                    } else {
                        statements.push(format!(
                            "-- [SAFE MODE SKIPPED] {}",
                            render_drop_index(&diff.name, &target_table_ident, report.family)
                        ));
                        warnings.push(format!(
                            "Index '{}' was not dropped because Safe Mode is enabled.",
                            diff.name
                        ));
                    }
                } else if let Some(idx) = &diff.target {
                    statements.push(render_create_index(idx, &target_table_ident, report.family));
                }
            }
            IndexDiffStatus::Modified => {
                let target_idx = if is_source_to_target {
                    diff.source.as_ref()
                } else {
                    diff.target.as_ref()
                };

                if let Some(idx) = target_idx {
                    // Recreate index: Drop then Create
                    if options.include_drops {
                        statements.push(render_drop_index(
                            &diff.name,
                            &target_table_ident,
                            report.family,
                        ));
                    } else {
                        statements.push(format!(
                            "-- [SAFE MODE SKIPPED] {}",
                            render_drop_index(&diff.name, &target_table_ident, report.family)
                        ));
                    }
                    statements.push(render_create_index(idx, &target_table_ident, report.family));
                }
            }
            IndexDiffStatus::Unchanged => {}
        }
    }

    // 3. Assemble Full Script
    let mut full_script = String::new();

    // Header comment
    full_script.push_str(&format!(
        "-- ====================================================================\n\
         -- Migration Script: {} -> {}\n\
         -- Target Table: {}\n\
         -- Database Dialect: {:?}\n\
         -- Generated by zqlcrab Visual Schema Diff Engine\n\
         -- ====================================================================\n\n",
        report.source_table, report.target_table, target_table_ident, report.family
    ));

    if sqlite_needs_rebuild {
        full_script.push_str(
            "-- NOTICE: SQLite requires table recreation for modified column types/constraints.\n\
             -- Review generated statements carefully before executing in production.\n\n",
        );
    }

    if statements.is_empty() {
        full_script.push_str("-- No schema discrepancies detected. Tables are identical.\n");
    } else {
        if options.wrap_transaction {
            match report.family {
                DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
                    full_script.push_str("BEGIN;\n\n");
                }
                DatabaseFamily::MySql => {
                    full_script.push_str("START TRANSACTION;\n\n");
                }
            }
        }

        for stmt in &statements {
            full_script.push_str(stmt);
            full_script.push('\n');
        }

        if options.wrap_transaction {
            full_script.push_str("\nCOMMIT;\n");
        }
    }

    MigrationScript {
        statements,
        full_script,
        warnings,
    }
}

// ---------------- Helper Functions ----------------

fn format_table_identifier(table: &str, schema: Option<&str>, family: DatabaseFamily) -> String {
    match schema {
        Some(s) if !s.trim().is_empty() && !s.eq_ignore_ascii_case("main") => {
            format!("{}.{}", quote_ident(s, family), quote_ident(table, family))
        }
        _ => quote_ident(table, family),
    }
}

fn render_column_definition(col: &ColumnInfo, family: DatabaseFamily) -> String {
    let mut parts = Vec::new();
    parts.push(quote_ident(&col.name, family));
    parts.push(col.data_type.clone());

    if !col.is_nullable {
        parts.push("NOT NULL".to_string());
    }

    if let Some(def) = &col.default_value {
        let trimmed = def.trim();
        if !trimmed.is_empty() {
            parts.push(format!("DEFAULT {}", trimmed));
        }
    }

    if col.is_auto_increment {
        match family {
            DatabaseFamily::MySql => parts.push("AUTO_INCREMENT".to_string()),
            DatabaseFamily::Postgres => {} // usually handled via SERIAL/IDENTITY
            DatabaseFamily::Sqlite => parts.push("PRIMARY KEY AUTOINCREMENT".to_string()),
        }
    }

    parts.join(" ")
}

fn render_create_index(idx: &IndexInfo, table_ident: &str, family: DatabaseFamily) -> String {
    let unique_str = if idx.is_unique { "UNIQUE " } else { "" };
    let cols_str = idx
        .columns
        .iter()
        .map(|c| quote_ident(c, family))
        .collect::<Vec<_>>()
        .join(", ");

    let idx_ident = quote_ident(&idx.name, family);

    match family {
        DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
            format!(
                "CREATE {}INDEX IF NOT EXISTS {} ON {} ({});",
                unique_str, idx_ident, table_ident, cols_str
            )
        }
        DatabaseFamily::MySql => {
            format!(
                "CREATE {}INDEX {} ON {} ({});",
                unique_str, idx_ident, table_ident, cols_str
            )
        }
    }
}

fn render_drop_index(name: &str, table_ident: &str, family: DatabaseFamily) -> String {
    let idx_ident = quote_ident(name, family);
    match family {
        DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
            format!("DROP INDEX IF EXISTS {};", idx_ident)
        }
        DatabaseFamily::MySql => {
            format!("DROP INDEX {} ON {};", idx_ident, table_ident)
        }
    }
}

fn types_match(t1: &str, t2: &str, _family: DatabaseFamily) -> bool {
    let s1 = normalize_type(t1);
    let s2 = normalize_type(t2);
    if s1 == s2 {
        return true;
    }

    // Common cross-synonyms
    match (s1.as_str(), s2.as_str()) {
        ("int", "integer") | ("integer", "int") => true,
        ("bool", "boolean") | ("boolean", "bool") => true,
        ("text", "clob") | ("clob", "text") => true,
        ("blob", "bytea") | ("bytea", "blob") => true,
        _ => false,
    }
}

fn normalize_type(t: &str) -> String {
    t.trim().to_lowercase().replace(" ", "")
}

fn defaults_match(d1: Option<&str>, d2: Option<&str>) -> bool {
    let clean = |opt: Option<&str>| -> Option<String> {
        opt.map(|s| {
            s.trim()
                .trim_matches('\'')
                .trim_matches('"')
                .trim_matches('(')
                .trim_matches(')')
                .to_lowercase()
        })
        .filter(|s| !s.is_empty() && s != "null")
    };

    clean(d1) == clean(d2)
}

fn index_columns_match(i1: &IndexInfo, i2: &IndexInfo) -> bool {
    if i1.columns.len() != i2.columns.len() {
        return false;
    }
    i1.columns
        .iter()
        .zip(i2.columns.iter())
        .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_col(name: &str, data_type: &str, nullable: bool, pk: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            is_nullable: nullable,
            is_primary_key: pk,
            is_auto_increment: false,
            default_value: None,
            description: None,
        }
    }

    #[test]
    fn test_identical_tables() {
        let cols = vec![
            sample_col("id", "INTEGER", false, true),
            sample_col("username", "VARCHAR(50)", false, false),
        ];
        let indexes = vec![IndexInfo {
            name: "idx_user".to_string(),
            table_name: "users".to_string(),
            columns: vec!["username".to_string()],
            is_unique: true,
            is_primary: false,
        }];

        let report = compare_tables(
            "users",
            None,
            &cols,
            &indexes,
            "users_backup",
            None,
            &cols,
            &indexes,
            DatabaseFamily::Postgres,
        );

        assert!(!report.summary.has_differences());
        assert_eq!(report.summary.unchanged_columns, 2);
        assert_eq!(report.summary.unchanged_indexes, 1);

        let script = generate_migration_ddl(&report, &SchemaDiffOptions::default());
        assert!(script.statements.is_empty());
        assert!(
            script
                .full_script
                .contains("No schema discrepancies detected")
        );
    }

    #[test]
    fn test_discrepancy_detection_and_migration() {
        let src_cols = vec![
            sample_col("id", "BIGINT", false, true),
            sample_col("email", "VARCHAR(255)", false, false),
            sample_col("created_at", "TIMESTAMP", true, false),
        ];

        let tgt_cols = vec![
            sample_col("id", "INTEGER", false, true), // Type difference
            sample_col("email", "VARCHAR(100)", false, false), // Type difference
            sample_col("legacy_code", "TEXT", true, false), // Dropped in source
        ];

        let src_indexes = vec![IndexInfo {
            name: "idx_email".to_string(),
            table_name: "users".to_string(),
            columns: vec!["email".to_string()],
            is_unique: true,
            is_primary: false,
        }];

        let report = compare_tables(
            "users",
            Some("public"),
            &src_cols,
            &src_indexes,
            "users_v2",
            Some("public"),
            &tgt_cols,
            &[],
            DatabaseFamily::Postgres,
        );

        assert_eq!(report.summary.added_columns, 1); // created_at
        assert_eq!(report.summary.modified_columns, 2); // id, email
        assert_eq!(report.summary.dropped_columns, 1); // legacy_code
        assert_eq!(report.summary.added_indexes, 1); // idx_email

        // Test with Safe Mode enabled (DROP is commented)
        let opts = SchemaDiffOptions {
            direction: MigrationDirection::SourceToTarget,
            include_drops: false,
            wrap_transaction: true,
        };
        let script = generate_migration_ddl(&report, &opts);

        assert!(script.full_script.contains("ADD COLUMN \"created_at\""));
        assert!(
            script
                .full_script
                .contains("ALTER COLUMN \"id\" TYPE BIGINT")
        );
        assert!(script.full_script.contains("SAFE MODE SKIPPED"));
        assert!(
            script
                .full_script
                .contains("CREATE UNIQUE INDEX IF NOT EXISTS \"idx_email\"")
        );
        assert!(script.full_script.contains("BEGIN;"));
        assert!(script.full_script.contains("COMMIT;"));
    }

    #[test]
    fn test_mysql_migration_syntax() {
        let src_cols = vec![sample_col("name", "VARCHAR(100)", false, false)];
        let tgt_cols = vec![sample_col("name", "VARCHAR(50)", true, false)];

        let report = compare_tables(
            "t1",
            None,
            &src_cols,
            &[],
            "t2",
            None,
            &tgt_cols,
            &[],
            DatabaseFamily::MySql,
        );

        let opts = SchemaDiffOptions::default();
        let script = generate_migration_ddl(&report, &opts);

        assert!(
            script
                .full_script
                .contains("ALTER TABLE `t2` MODIFY COLUMN `name` VARCHAR(100) NOT NULL;")
        );
        assert!(script.full_script.contains("START TRANSACTION;"));
    }
}
