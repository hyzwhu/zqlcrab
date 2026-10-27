//! Visual Table Designer and DDL Schema Migration Generator.
//!
//! Provides data structures and multi-dialect SQL generation for creating new tables
//! and altering existing tables across PostgreSQL, MySQL, and SQLite.
//! Tracks granular column modifications, renames, additions, deletions, index adjustments,
//! and foreign key constraints, producing clean, executable DDL migration scripts.

use crate::db::types::{ColumnInfo, DatabaseFamily, ForeignKeyInfo, IndexInfo, TableInfo, quote_ident};
use serde::{Deserialize, Serialize};

/// Represents a column in the visual table designer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignerColumn {
    /// Internal stable identifier for UI element keys and drag/drop tracking.
    pub id: String,
    /// Column name in the schema.
    pub name: String,
    /// Original column name if this column was loaded from an existing table (for rename detection).
    pub original_name: Option<String>,
    /// Column data type specification (e.g. `VARCHAR(255)`, `BIGINT`, `TEXT`).
    pub data_type: String,
    /// Original data type when loaded from existing table.
    pub original_data_type: Option<String>,
    /// Whether this column is part of the primary key.
    pub is_primary_key: bool,
    /// Original primary key status.
    pub original_is_primary_key: bool,
    /// Whether this column prohibits NULL values.
    pub not_null: bool,
    /// Original not_null status.
    pub original_not_null: bool,
    /// Default value expression (e.g. `'active'`, `CURRENT_TIMESTAMP`, `0`).
    pub default_value: Option<String>,
    /// Original default value.
    pub original_default_value: Option<String>,
    /// Column comment / documentation description.
    pub comment: Option<String>,
    /// Original column comment.
    pub original_comment: Option<String>,
    /// Whether the column is auto-incrementing / identity.
    pub auto_increment: bool,
    /// Flag indicating whether the column was marked for deletion.
    pub is_deleted: bool,
}

impl DesignerColumn {
    /// Creates a new designer column with default attributes.
    pub fn new(id: impl Into<String>, name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            original_name: None,
            data_type: data_type.into(),
            original_data_type: None,
            is_primary_key: false,
            original_is_primary_key: false,
            not_null: false,
            original_not_null: false,
            default_value: None,
            original_default_value: None,
            comment: None,
            original_comment: None,
            auto_increment: false,
            is_deleted: false,
        }
    }

    /// Converts an existing `ColumnInfo` into a `DesignerColumn`.
    pub fn from_existing(id: impl Into<String>, col: &ColumnInfo) -> Self {
        let name = col.name.clone();
        let dt = col.data_type.clone();
        let pk = col.is_primary_key;
        let not_null = !col.is_nullable;
        let def = col.default_value.clone();
        let comment = col.description.clone();

        Self {
            id: id.into(),
            name: name.clone(),
            original_name: Some(name),
            data_type: dt.clone(),
            original_data_type: Some(dt),
            is_primary_key: pk,
            original_is_primary_key: pk,
            not_null,
            original_not_null: not_null,
            default_value: def.clone(),
            original_default_value: def,
            comment: comment.clone(),
            original_comment: comment,
            auto_increment: col.is_auto_increment,
            is_deleted: false,
        }
    }

    /// Returns true if this column was modified compared to its original state.
    pub fn is_modified(&self) -> bool {
        if self.original_name.is_none() {
            return false; // brand new column
        }
        self.is_renamed()
            || self.original_data_type.as_deref() != Some(&self.data_type)
            || self.original_is_primary_key != self.is_primary_key
            || self.original_not_null != self.not_null
            || self.original_default_value != self.default_value
            || self.original_comment != self.comment
    }

    /// Returns true if the column was renamed.
    pub fn is_renamed(&self) -> bool {
        match &self.original_name {
            Some(orig) => orig != &self.name,
            None => false,
        }
    }
}

/// Represents an index definition in the visual table designer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignerIndex {
    pub id: String,
    pub name: String,
    pub original_name: Option<String>,
    pub columns: Vec<String>,
    pub original_columns: Option<Vec<String>>,
    pub is_unique: bool,
    pub original_is_unique: bool,
    pub is_deleted: bool,
}

impl DesignerIndex {
    pub fn new(id: impl Into<String>, name: impl Into<String>, columns: Vec<String>, is_unique: bool) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            original_name: None,
            columns,
            original_columns: None,
            is_unique,
            original_is_unique: false,
            is_deleted: false,
        }
    }

    pub fn from_existing(id: impl Into<String>, idx: &IndexInfo) -> Self {
        let name = idx.name.clone();
        let cols = idx.columns.clone();
        let unq = idx.is_unique;

        Self {
            id: id.into(),
            name: name.clone(),
            original_name: Some(name),
            columns: cols.clone(),
            original_columns: Some(cols),
            is_unique: unq,
            original_is_unique: unq,
            is_deleted: false,
        }
    }
}

/// Represents a foreign key definition in the visual table designer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignerForeignKey {
    pub id: String,
    pub name: String,
    pub original_name: Option<String>,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
    pub is_deleted: bool,
}

impl DesignerForeignKey {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        columns: Vec<String>,
        referenced_table: impl Into<String>,
        referenced_columns: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            original_name: None,
            columns,
            referenced_table: referenced_table.into(),
            referenced_columns,
            on_update: None,
            on_delete: None,
            is_deleted: false,
        }
    }

    pub fn from_existing(id: impl Into<String>, fk: &ForeignKeyInfo) -> Self {
        let name = fk.name.clone();
        Self {
            id: id.into(),
            name: name.clone(),
            original_name: Some(name),
            columns: fk.columns.clone(),
            referenced_table: fk.referenced_table.clone(),
            referenced_columns: fk.referenced_columns.clone(),
            on_update: fk.on_update.clone(),
            on_delete: fk.on_delete.clone(),
            is_deleted: false,
        }
    }
}

/// Active model representing a table in the Table Designer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignerTable {
    /// Current table name.
    pub name: String,
    /// Original table name (if modifying an existing table).
    pub original_name: Option<String>,
    /// Schema or database name qualifier.
    pub schema: Option<String>,
    /// Table comment or documentation description.
    pub comment: Option<String>,
    /// Original comment.
    pub original_comment: Option<String>,
    /// Ordered list of designer columns.
    pub columns: Vec<DesignerColumn>,
    /// Index definitions.
    pub indexes: Vec<DesignerIndex>,
    /// Foreign key definitions.
    pub foreign_keys: Vec<DesignerForeignKey>,
    /// Whether this designer session represents creating a brand new table.
    pub is_new_table: bool,
}

impl DesignerTable {
    /// Creates a blank designer table model for creating a new table.
    pub fn new_table(default_name: &str, schema: Option<String>) -> Self {
        let initial_columns = vec![
            DesignerColumn {
                id: "col_id".to_string(),
                name: "id".to_string(),
                original_name: None,
                data_type: "BIGINT".to_string(),
                original_data_type: None,
                is_primary_key: true,
                original_is_primary_key: false,
                not_null: true,
                original_not_null: false,
                default_value: None,
                original_default_value: None,
                comment: Some("Primary Key".to_string()),
                original_comment: None,
                auto_increment: true,
                is_deleted: false,
            },
            DesignerColumn {
                id: "col_created_at".to_string(),
                name: "created_at".to_string(),
                original_name: None,
                data_type: "TIMESTAMP".to_string(),
                original_data_type: None,
                is_primary_key: false,
                original_is_primary_key: false,
                not_null: true,
                original_not_null: false,
                default_value: Some("CURRENT_TIMESTAMP".to_string()),
                original_default_value: None,
                comment: None,
                original_comment: None,
                auto_increment: false,
                is_deleted: false,
            },
        ];

        Self {
            name: default_name.to_string(),
            original_name: None,
            schema,
            comment: None,
            original_comment: None,
            columns: initial_columns,
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
            is_new_table: true,
        }
    }

    /// Loads an existing table and its schema into the designer model.
    pub fn from_existing(
        table: &TableInfo,
        columns: &[ColumnInfo],
        indexes: &[IndexInfo],
        foreign_keys: &[ForeignKeyInfo],
    ) -> Self {
        let des_cols = columns
            .iter()
            .enumerate()
            .map(|(i, c)| DesignerColumn::from_existing(format!("col_{i}"), c))
            .collect();

        let des_indexes = indexes
            .iter()
            .enumerate()
            .filter(|(_, idx)| !idx.is_primary) // Primary keys handled in column attributes
            .map(|(i, idx)| DesignerIndex::from_existing(format!("idx_{i}"), idx))
            .collect();

        let des_fks = foreign_keys
            .iter()
            .enumerate()
            .map(|(i, fk)| DesignerForeignKey::from_existing(format!("fk_{i}"), fk))
            .collect();

        Self {
            name: table.name.clone(),
            original_name: Some(table.name.clone()),
            schema: table.schema.clone(),
            comment: table.comment.clone(),
            original_comment: table.comment.clone(),
            columns: des_cols,
            indexes: des_indexes,
            foreign_keys: des_fks,
            is_new_table: false,
        }
    }

    /// Returns all active (non-deleted) columns.
    pub fn active_columns(&self) -> Vec<&DesignerColumn> {
        self.columns.iter().filter(|c| !c.is_deleted).collect()
    }

    /// Returns all active (non-deleted) indexes.
    pub fn active_indexes(&self) -> Vec<&DesignerIndex> {
        self.indexes.iter().filter(|i| !i.is_deleted).collect()
    }

    /// Returns all active (non-deleted) foreign keys.
    pub fn active_foreign_keys(&self) -> Vec<&DesignerForeignKey> {
        self.foreign_keys.iter().filter(|f| !f.is_deleted).collect()
    }

    /// Validates table design specifications, returning any validation error messages.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            errors.push("Table name cannot be empty.".to_string());
        }

        let active_cols = self.active_columns();
        if active_cols.is_empty() {
            errors.push("Table must have at least one active column.".to_string());
        }

        let mut seen_col_names = std::collections::HashSet::new();
        for col in &active_cols {
            let col_name_trimmed = col.name.trim();
            if col_name_trimmed.is_empty() {
                errors.push(format!("Column with ID '{}' has an empty name.", col.id));
            } else if !seen_col_names.insert(col_name_trimmed.to_lowercase()) {
                errors.push(format!("Duplicate column name '{}' detected.", col.name));
            }

            if col.data_type.trim().is_empty() {
                errors.push(format!("Column '{}' must specify a data type.", col.name));
            }
        }

        for idx in self.active_indexes() {
            if idx.name.trim().is_empty() {
                errors.push("Index name cannot be empty.".to_string());
            }
            if idx.columns.is_empty() {
                errors.push(format!("Index '{}' must specify at least one column.", idx.name));
            }
        }

        for fk in self.active_foreign_keys() {
            if fk.columns.is_empty() {
                errors.push(format!("Foreign key '{}' must specify source columns.", fk.name));
            }
            if fk.referenced_table.trim().is_empty() {
                errors.push(format!("Foreign key '{}' must specify a referenced table.", fk.name));
            }
            if fk.referenced_columns.is_empty() {
                errors.push(format!("Foreign key '{}' must specify referenced columns.", fk.name));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Computes summary counts of planned schema modifications.
    pub fn diff_summary(&self) -> TableDiffSummary {
        if self.is_new_table {
            return TableDiffSummary {
                is_create: true,
                columns_added: self.active_columns().len(),
                columns_dropped: 0,
                columns_modified: 0,
                indexes_added: self.active_indexes().len(),
                indexes_dropped: 0,
                fks_added: self.active_foreign_keys().len(),
                fks_dropped: 0,
                renamed_table: false,
            };
        }

        let mut summary = TableDiffSummary {
            is_create: false,
            renamed_table: self.original_name.as_deref() != Some(&self.name),
            ..Default::default()
        };

        for col in &self.columns {
            if col.is_deleted && col.original_name.is_some() {
                summary.columns_dropped += 1;
            } else if !col.is_deleted && col.original_name.is_none() {
                summary.columns_added += 1;
            } else if !col.is_deleted && col.is_modified() {
                summary.columns_modified += 1;
            }
        }

        for idx in &self.indexes {
            if idx.is_deleted && idx.original_name.is_some() {
                summary.indexes_dropped += 1;
            } else if !idx.is_deleted && idx.original_name.is_none() {
                summary.indexes_added += 1;
            }
        }

        for fk in &self.foreign_keys {
            if fk.is_deleted && fk.original_name.is_some() {
                summary.fks_dropped += 1;
            } else if !fk.is_deleted && fk.original_name.is_none() {
                summary.fks_added += 1;
            }
        }

        summary
    }

    /// Generates multi-dialect SQL statements according to the target database family.
    pub fn generate_ddl(&self, family: DatabaseFamily) -> Result<String, String> {
        self.validate().map_err(|errs| errs.join("\n"))?;

        if self.is_new_table {
            self.generate_create_table_ddl(family)
        } else {
            self.generate_alter_table_ddl(family)
        }
    }

    fn qualified_name(&self, family: DatabaseFamily, table_name: &str) -> String {
        match self.schema.as_deref() {
            Some(s) if !s.is_empty() => {
                format!("{}.{}", quote_ident(s, family), quote_ident(table_name, family))
            }
            _ => quote_ident(table_name, family),
        }
    }

    /// Generates CREATE TABLE statement with inline constraints, indexes, and dialect-specific formatting.
    fn generate_create_table_ddl(&self, family: DatabaseFamily) -> Result<String, String> {
        let mut stmts = Vec::new();
        let target_tbl = self.qualified_name(family, &self.name);
        let active_cols = self.active_columns();

        let mut col_defs = Vec::new();
        let mut pk_cols = Vec::new();

        for col in &active_cols {
            let col_name_q = quote_ident(&col.name, family);
            let mut def = format!("  {} {}", col_name_q, col.data_type);

            if col.auto_increment {
                match family {
                    DatabaseFamily::Postgres => {
                        if col.data_type.to_uppercase() == "BIGINT" {
                            def = format!("  {} BIGSERIAL", col_name_q);
                        } else if col.data_type.to_uppercase() == "INT"
                            || col.data_type.to_uppercase() == "INTEGER"
                        {
                            def = format!("  {} SERIAL", col_name_q);
                        } else {
                            def.push_str(" GENERATED BY DEFAULT AS IDENTITY");
                        }
                    }
                    DatabaseFamily::MySql => {
                        def.push_str(" AUTO_INCREMENT");
                    }
                    DatabaseFamily::Sqlite => {
                        if col.is_primary_key {
                            def = format!("  {} INTEGER PRIMARY KEY AUTOINCREMENT", col_name_q);
                        }
                    }
                }
            }

            if col.not_null && (!col.auto_increment || family != DatabaseFamily::Sqlite) {
                def.push_str(" NOT NULL");
            }

            if let Some(ref val) = col.default_value {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    def.push_str(&format!(" DEFAULT {trimmed}"));
                }
            }

            if family == DatabaseFamily::MySql {
                if let Some(ref comment) = col.comment {
                    let escaped = comment.replace('\'', "''");
                    def.push_str(&format!(" COMMENT '{escaped}'"));
                }
            }

            if col.is_primary_key && !(family == DatabaseFamily::Sqlite && col.auto_increment) {
                pk_cols.push(col.name.clone());
            }

            col_defs.push(def);
        }

        if !pk_cols.is_empty() && !(family == DatabaseFamily::Sqlite && pk_cols.len() == 1 && active_cols.iter().any(|c| c.name == pk_cols[0] && c.auto_increment)) {
            let pk_list: Vec<String> = pk_cols.iter().map(|c| quote_ident(c, family)).collect();
            col_defs.push(format!("  PRIMARY KEY ({})", pk_list.join(", ")));
        }

        // Inline foreign keys if applicable
        for fk in self.active_foreign_keys() {
            let src_cols: Vec<String> = fk.columns.iter().map(|c| quote_ident(c, family)).collect();
            let ref_cols: Vec<String> = fk.referenced_columns.iter().map(|c| quote_ident(c, family)).collect();
            let ref_tbl = quote_ident(&fk.referenced_table, family);
            let mut fk_def = format!(
                "  CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({})",
                quote_ident(&fk.name, family),
                src_cols.join(", "),
                ref_tbl,
                ref_cols.join(", ")
            );
            if let Some(ref u) = fk.on_update {
                fk_def.push_str(&format!(" ON UPDATE {u}"));
            }
            if let Some(ref d) = fk.on_delete {
                fk_def.push_str(&format!(" ON DELETE {d}"));
            }
            col_defs.push(fk_def);
        }

        let mut create_table = format!("CREATE TABLE {} (\n{}\n)", target_tbl, col_defs.join(",\n"));
        if family == DatabaseFamily::MySql {
            create_table.push_str(" ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci");
            if let Some(ref tbl_cmt) = self.comment {
                let escaped = tbl_cmt.replace('\'', "''");
                create_table.push_str(&format!(" COMMENT='{escaped}'"));
            }
        }
        create_table.push(';');
        stmts.push(create_table);

        // Table and column comments for PostgreSQL
        if family == DatabaseFamily::Postgres {
            if let Some(ref tbl_cmt) = self.comment {
                let escaped = tbl_cmt.replace('\'', "''");
                stmts.push(format!("COMMENT ON TABLE {} IS '{escaped}';", target_tbl));
            }
            for col in &active_cols {
                if let Some(ref col_cmt) = col.comment {
                    let escaped = col_cmt.replace('\'', "''");
                    stmts.push(format!(
                        "COMMENT ON COLUMN {}.{} IS '{escaped}';",
                        target_tbl,
                        quote_ident(&col.name, family)
                    ));
                }
            }
        }

        // Separate index creation
        for idx in self.active_indexes() {
            let unique_kw = if idx.is_unique { "UNIQUE " } else { "" };
            let idx_name = quote_ident(&idx.name, family);
            let idx_cols: Vec<String> = idx.columns.iter().map(|c| quote_ident(c, family)).collect();
            stmts.push(format!(
                "CREATE {}INDEX {} ON {} ({});",
                unique_kw,
                idx_name,
                target_tbl,
                idx_cols.join(", ")
            ));
        }

        Ok(stmts.join("\n\n"))
    }

    /// Generates granular ALTER TABLE migration statements for an existing table.
    fn generate_alter_table_ddl(&self, family: DatabaseFamily) -> Result<String, String> {
        let mut stmts = Vec::new();
        let orig_name = self.original_name.as_deref().unwrap_or(&self.name);
        let mut current_tbl = self.qualified_name(family, orig_name);

        // 1. Rename table
        if self.name != orig_name {
            let new_tbl_q = quote_ident(&self.name, family);
            stmts.push(format!("ALTER TABLE {} RENAME TO {};", current_tbl, new_tbl_q));
            current_tbl = self.qualified_name(family, &self.name);
        }

        // 2. Drop Foreign Keys first
        for fk in &self.foreign_keys {
            if fk.is_deleted && fk.original_name.is_some() {
                let fk_name = quote_ident(fk.original_name.as_ref().unwrap(), family);
                match family {
                    DatabaseFamily::Postgres => {
                        stmts.push(format!("ALTER TABLE {} DROP CONSTRAINT {};", current_tbl, fk_name));
                    }
                    DatabaseFamily::MySql => {
                        stmts.push(format!("ALTER TABLE {} DROP FOREIGN KEY {};", current_tbl, fk_name));
                    }
                    DatabaseFamily::Sqlite => {
                        stmts.push(format!("-- SQLite foreign keys cannot be dropped directly via ALTER TABLE: {}", fk.name));
                    }
                }
            }
        }

        // 3. Drop Indexes
        for idx in &self.indexes {
            if idx.is_deleted && idx.original_name.is_some() {
                let idx_name = quote_ident(idx.original_name.as_ref().unwrap(), family);
                match family {
                    DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
                        stmts.push(format!("DROP INDEX {};", idx_name));
                    }
                    DatabaseFamily::MySql => {
                        stmts.push(format!("ALTER TABLE {} DROP INDEX {};", current_tbl, idx_name));
                    }
                }
            }
        }

        // 4. Drop Columns
        for col in &self.columns {
            if col.is_deleted && col.original_name.is_some() {
                let col_name_q = quote_ident(col.original_name.as_ref().unwrap(), family);
                stmts.push(format!("ALTER TABLE {} DROP COLUMN {};", current_tbl, col_name_q));
            }
        }

        // 5. Rename Columns
        for col in &self.columns {
            if !col.is_deleted && col.is_renamed() {
                if let Some(ref old_name) = col.original_name {
                    let old_q = quote_ident(old_name, family);
                    let new_q = quote_ident(&col.name, family);
                    match family {
                        DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
                            stmts.push(format!(
                                "ALTER TABLE {} RENAME COLUMN {} TO {};",
                                current_tbl, old_q, new_q
                            ));
                        }
                        DatabaseFamily::MySql => {
                            stmts.push(format!(
                                "ALTER TABLE {} RENAME COLUMN {} TO {};",
                                current_tbl, old_q, new_q
                            ));
                        }
                    }
                }
            }
        }

        // 6. Add Columns
        for col in &self.columns {
            if !col.is_deleted && col.original_name.is_none() {
                let col_name_q = quote_ident(&col.name, family);
                let mut def = format!("ALTER TABLE {} ADD COLUMN {} {}", current_tbl, col_name_q, col.data_type);

                if col.not_null {
                    def.push_str(" NOT NULL");
                }
                if let Some(ref val) = col.default_value {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        def.push_str(&format!(" DEFAULT {trimmed}"));
                    }
                }
                if family == DatabaseFamily::MySql {
                    if let Some(ref cmt) = col.comment {
                        let escaped = cmt.replace('\'', "''");
                        def.push_str(&format!(" COMMENT '{escaped}'"));
                    }
                }
                def.push(';');
                stmts.push(def);

                if family == DatabaseFamily::Postgres {
                    if let Some(ref cmt) = col.comment {
                        let escaped = cmt.replace('\'', "''");
                        stmts.push(format!(
                            "COMMENT ON COLUMN {}.{} IS '{escaped}';",
                            current_tbl, col_name_q
                        ));
                    }
                }
            }
        }

        // 7. Modify Column Types, Nullability, Defaults
        for col in &self.columns {
            if !col.is_deleted && col.original_name.is_some() && col.is_modified() {
                let col_name_q = quote_ident(&col.name, family);

                match family {
                    DatabaseFamily::Postgres => {
                        // Data type alteration
                        if col.original_data_type.as_deref() != Some(&col.data_type) {
                            stmts.push(format!(
                                "ALTER TABLE {} ALTER COLUMN {} TYPE {};",
                                current_tbl, col_name_q, col.data_type
                            ));
                        }
                        // Nullability alteration
                        if col.original_not_null != col.not_null {
                            if col.not_null {
                                stmts.push(format!(
                                    "ALTER TABLE {} ALTER COLUMN {} SET NOT NULL;",
                                    current_tbl, col_name_q
                                ));
                            } else {
                                stmts.push(format!(
                                    "ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL;",
                                    current_tbl, col_name_q
                                ));
                            }
                        }
                        // Default value alteration
                        if col.original_default_value != col.default_value {
                            if let Some(ref def_val) = col.default_value {
                                if !def_val.trim().is_empty() {
                                    stmts.push(format!(
                                        "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {};",
                                        current_tbl, col_name_q, def_val.trim()
                                    ));
                                } else {
                                    stmts.push(format!(
                                        "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT;",
                                        current_tbl, col_name_q
                                    ));
                                }
                            } else {
                                stmts.push(format!(
                                    "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT;",
                                    current_tbl, col_name_q
                                ));
                            }
                        }
                        // Comment alteration
                        if col.original_comment != col.comment {
                            if let Some(ref cmt) = col.comment {
                                let escaped = cmt.replace('\'', "''");
                                stmts.push(format!(
                                    "COMMENT ON COLUMN {}.{} IS '{escaped}';",
                                    current_tbl, col_name_q
                                ));
                            } else {
                                stmts.push(format!(
                                    "COMMENT ON COLUMN {}.{} IS NULL;",
                                    current_tbl, col_name_q
                                ));
                            }
                        }
                    }
                    DatabaseFamily::MySql => {
                        let mut modify_def = format!(
                            "ALTER TABLE {} MODIFY COLUMN {} {}",
                            current_tbl, col_name_q, col.data_type
                        );
                        if col.not_null {
                            modify_def.push_str(" NOT NULL");
                        }
                        if let Some(ref val) = col.default_value {
                            let trimmed = val.trim();
                            if !trimmed.is_empty() {
                                modify_def.push_str(&format!(" DEFAULT {trimmed}"));
                            }
                        }
                        if let Some(ref cmt) = col.comment {
                            let escaped = cmt.replace('\'', "''");
                            modify_def.push_str(&format!(" COMMENT '{escaped}'"));
                        }
                        modify_def.push(';');
                        stmts.push(modify_def);
                    }
                    DatabaseFamily::Sqlite => {
                        stmts.push(format!(
                            "-- SQLite does not support in-place column type/nullability alteration for: {}",
                            col.name
                        ));
                    }
                }
            }
        }

        // 8. Add Indexes
        for idx in &self.indexes {
            if !idx.is_deleted && idx.original_name.is_none() {
                let unique_kw = if idx.is_unique { "UNIQUE " } else { "" };
                let idx_name = quote_ident(&idx.name, family);
                let idx_cols: Vec<String> = idx.columns.iter().map(|c| quote_ident(c, family)).collect();
                stmts.push(format!(
                    "CREATE {}INDEX {} ON {} ({});",
                    unique_kw,
                    idx_name,
                    current_tbl,
                    idx_cols.join(", ")
                ));
            }
        }

        // 9. Add Foreign Keys
        for fk in &self.foreign_keys {
            if !fk.is_deleted && fk.original_name.is_none() {
                let src_cols: Vec<String> = fk.columns.iter().map(|c| quote_ident(c, family)).collect();
                let ref_cols: Vec<String> = fk.referenced_columns.iter().map(|c| quote_ident(c, family)).collect();
                let ref_tbl = quote_ident(&fk.referenced_table, family);
                let mut fk_def = format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({})",
                    current_tbl,
                    quote_ident(&fk.name, family),
                    src_cols.join(", "),
                    ref_tbl,
                    ref_cols.join(", ")
                );
                if let Some(ref u) = fk.on_update {
                    fk_def.push_str(&format!(" ON UPDATE {u}"));
                }
                if let Some(ref d) = fk.on_delete {
                    fk_def.push_str(&format!(" ON DELETE {d}"));
                }
                fk_def.push(';');
                stmts.push(fk_def);
            }
        }

        // 10. Table comment change
        if self.original_comment != self.comment {
            if family == DatabaseFamily::Postgres {
                if let Some(ref cmt) = self.comment {
                    let escaped = cmt.replace('\'', "''");
                    stmts.push(format!("COMMENT ON TABLE {} IS '{escaped}';", current_tbl));
                } else {
                    stmts.push(format!("COMMENT ON TABLE {} IS NULL;", current_tbl));
                }
            } else if family == DatabaseFamily::MySql {
                let cmt_str = self.comment.as_deref().unwrap_or("");
                let escaped = cmt_str.replace('\'', "''");
                stmts.push(format!("ALTER TABLE {} COMMENT='{escaped}';", current_tbl));
            }
        }

        if stmts.is_empty() {
            Ok("-- No structural changes detected.".to_string())
        } else {
            Ok(stmts.join("\n\n"))
        }
    }
}

/// Summarizes total counts of schema modifications in the current designer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableDiffSummary {
    pub is_create: bool,
    pub columns_added: usize,
    pub columns_dropped: usize,
    pub columns_modified: usize,
    pub indexes_added: usize,
    pub indexes_dropped: usize,
    pub fks_added: usize,
    pub fks_dropped: usize,
    pub renamed_table: bool,
}

impl TableDiffSummary {
    pub fn has_changes(&self) -> bool {
        self.is_create
            || self.renamed_table
            || self.columns_added > 0
            || self.columns_dropped > 0
            || self.columns_modified > 0
            || self.indexes_added > 0
            || self.indexes_dropped > 0
            || self.fks_added > 0
            || self.fks_dropped > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_table_postgres() {
        let mut tbl = DesignerTable::new_table("orders", Some("public".to_string()));
        tbl.columns.push(DesignerColumn {
            id: "col_price".to_string(),
            name: "total_amount".to_string(),
            original_name: None,
            data_type: "DECIMAL(12,2)".to_string(),
            original_data_type: None,
            is_primary_key: false,
            original_is_primary_key: false,
            not_null: true,
            original_not_null: false,
            default_value: Some("0.00".to_string()),
            original_default_value: None,
            comment: Some("Total order cost".to_string()),
            original_comment: None,
            auto_increment: false,
            is_deleted: false,
        });

        tbl.indexes.push(DesignerIndex::new(
            "idx_created",
            "idx_orders_created",
            vec!["created_at".to_string()],
            false,
        ));

        let ddl = tbl.generate_ddl(DatabaseFamily::Postgres).unwrap();
        assert!(ddl.contains("CREATE TABLE \"public\".\"orders\""));
        assert!(ddl.contains("\"id\" BIGSERIAL"));
        assert!(ddl.contains("\"total_amount\" DECIMAL(12,2) NOT NULL DEFAULT 0.00"));
        assert!(ddl.contains("PRIMARY KEY (\"id\")"));
        assert!(ddl.contains("CREATE INDEX \"idx_orders_created\" ON \"public\".\"orders\" (\"created_at\");"));
        assert!(ddl.contains("COMMENT ON COLUMN \"public\".\"orders\".\"total_amount\" IS 'Total order cost';"));
    }

    #[test]
    fn test_create_table_mysql() {
        let mut tbl = DesignerTable::new_table("users", None);
        tbl.comment = Some("System user accounts".to_string());
        tbl.columns[0].data_type = "INT".to_string();

        let ddl = tbl.generate_ddl(DatabaseFamily::MySql).unwrap();
        assert!(ddl.contains("CREATE TABLE `users`"));
        assert!(ddl.contains("`id` INT AUTO_INCREMENT NOT NULL"));
        assert!(ddl.contains("PRIMARY KEY (`id`)"));
        assert!(ddl.contains("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"));
        assert!(ddl.contains("COMMENT='System user accounts'"));
    }

    #[test]
    fn test_create_table_sqlite() {
        let tbl = DesignerTable::new_table("items", None);
        let ddl = tbl.generate_ddl(DatabaseFamily::Sqlite).unwrap();
        assert!(ddl.contains("CREATE TABLE \"items\""));
        assert!(ddl.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"));
    }

    #[test]
    fn test_alter_table_postgres() {
        let orig_table = TableInfo {
            name: "customers".to_string(),
            schema: Some("public".to_string()),
            table_type: "TABLE".to_string(),
            comment: None,
            row_count_estimate: None,
        };
        let orig_cols = vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "BIGINT".to_string(),
                is_nullable: false,
                default_value: None,
                is_primary_key: true,
                is_auto_increment: true,
                description: None,
            },
            ColumnInfo {
                name: "email".to_string(),
                data_type: "VARCHAR(100)".to_string(),
                is_nullable: false,
                default_value: None,
                is_primary_key: false,
                is_auto_increment: false,
                description: None,
            },
            ColumnInfo {
                name: "legacy_notes".to_string(),
                data_type: "TEXT".to_string(),
                is_nullable: true,
                default_value: None,
                is_primary_key: false,
                is_auto_increment: false,
                description: None,
            },
        ];

        let mut des = DesignerTable::from_existing(&orig_table, &orig_cols, &[], &[]);

        // 1. Rename table
        des.name = "clients".to_string();
        // 2. Rename email -> contact_email
        des.columns[1].name = "contact_email".to_string();
        // 3. Drop legacy_notes
        des.columns[2].is_deleted = true;
        // 4. Add status column
        des.columns.push(DesignerColumn {
            id: "col_status".to_string(),
            name: "status".to_string(),
            original_name: None,
            data_type: "VARCHAR(30)".to_string(),
            original_data_type: None,
            is_primary_key: false,
            original_is_primary_key: false,
            not_null: true,
            original_not_null: false,
            default_value: Some("'active'".to_string()),
            original_default_value: None,
            comment: None,
            original_comment: None,
            auto_increment: false,
            is_deleted: false,
        });

        let summary = des.diff_summary();
        assert!(summary.renamed_table);
        assert_eq!(summary.columns_dropped, 1);
        assert_eq!(summary.columns_added, 1);

        let ddl = des.generate_ddl(DatabaseFamily::Postgres).unwrap();
        assert!(ddl.contains("ALTER TABLE \"public\".\"customers\" RENAME TO \"clients\";"));
        assert!(ddl.contains("ALTER TABLE \"public\".\"clients\" DROP COLUMN \"legacy_notes\";"));
        assert!(ddl.contains("ALTER TABLE \"public\".\"clients\" RENAME COLUMN \"email\" TO \"contact_email\";"));
        assert!(ddl.contains("ALTER TABLE \"public\".\"clients\" ADD COLUMN \"status\" VARCHAR(30) NOT NULL DEFAULT 'active';"));
    }

    #[test]
    fn test_validation_errors() {
        let mut tbl = DesignerTable::new_table("", None);
        tbl.columns.clear();

        let err = tbl.validate().unwrap_err();
        assert!(err.iter().any(|e| e.contains("Table name cannot be empty")));
        assert!(err.iter().any(|e| e.contains("at least one active column")));
    }
}
