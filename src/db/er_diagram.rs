//! Visual ER Diagram graph model, layout engine, and Mermaid exporter.
//!
//! Provides relational topology discovery, automatic 2D graph positioning,
//! table card geometry calculation, and standard Mermaid ER diagram generation.

use crate::db::types::{ColumnInfo, ForeignKeyInfo, IndexInfo};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// Cardinality relationship between two tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationCardinality {
    /// 1 : 1 relationship (e.g. user to user_profile where foreign key is unique)
    OneToOne,
    /// 1 : N relationship (standard foreign key reference)
    OneToMany,
    /// N : 1 relationship (inverse of OneToMany)
    ManyToOne,
}

impl RelationCardinality {
    pub fn badge_label(&self) -> &'static str {
        match self {
            Self::OneToOne => "1 : 1",
            Self::OneToMany => "1 : N",
            Self::ManyToOne => "N : 1",
        }
    }

    /// Mermaid notation for entity relationship.
    pub fn mermaid_arrow(&self) -> &'static str {
        match self {
            Self::OneToOne => "||--||",
            Self::OneToMany => "||--o{",
            Self::ManyToOne => "}o--||",
        }
    }
}

/// An individual relational connection (foreign key edge) between two tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationEdge {
    pub id: String,
    pub constraint_name: String,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_table: String,
    pub to_columns: Vec<String>,
    pub cardinality: RelationCardinality,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}

/// A table node in the visual ER graph layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableNode {
    pub name: String,
    pub schema: Option<String>,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub row_count_estimate: Option<u64>,
    pub is_view: bool,
    // 2D Canvas positioning & bounds
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl TableNode {
    pub fn new(name: &str, schema: Option<&str>, is_view: bool) -> Self {
        Self {
            name: name.to_string(),
            schema: schema.map(ToString::to_string),
            columns: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
            row_count_estimate: None,
            is_view,
            x: 0.0,
            y: 0.0,
            width: 270.0,
            height: 120.0,
        }
    }

    pub fn display_name(&self) -> &str {
        &self.name
    }

    /// Count of primary key columns.
    pub fn primary_key_count(&self) -> usize {
        self.columns.iter().filter(|c| c.is_primary_key).count()
    }

    /// Check if a column by name is a primary key.
    pub fn is_primary_key(&self, col_name: &str) -> bool {
        self.columns
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(col_name) && c.is_primary_key)
    }

    /// Check if a column by name is part of any foreign key.
    pub fn is_foreign_key(&self, col_name: &str) -> bool {
        self.foreign_keys
            .iter()
            .any(|fk| fk.columns.iter().any(|c| c.eq_ignore_ascii_case(col_name)))
    }

    /// Recalculate estimated height based on column count.
    pub fn update_dimensions(&mut self) {
        let base_header_footer = 72.0;
        let row_height = 24.0;
        let displayed_cols = self.columns.len().min(12);
        let extra_cols_indicator = if self.columns.len() > 12 { 24.0 } else { 0.0 };
        self.width = 270.0;
        self.height =
            base_header_footer + (displayed_cols as f32 * row_height) + extra_cols_indicator;
    }
}

/// Full ER diagram graph model encompassing tables, connections, and layout coordinates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ErDiagramGraph {
    pub tables: Vec<TableNode>,
    pub relations: Vec<RelationEdge>,
    pub canvas_width: f32,
    pub canvas_height: f32,
}

impl ErDiagramGraph {
    /// Construct an ER diagram graph from table nodes and foreign key metadata.
    pub fn from_metadata(mut tables: Vec<TableNode>, foreign_keys: &[ForeignKeyInfo]) -> Self {
        let mut relations = Vec::new();

        // 1. Associate foreign keys with their source tables
        for fk in foreign_keys {
            if let Some(tbl) = tables
                .iter_mut()
                .find(|t| t.name.eq_ignore_ascii_case(&fk.table_name))
            {
                if !tbl
                    .foreign_keys
                    .iter()
                    .any(|existing| existing.name == fk.name)
                {
                    tbl.foreign_keys.push(fk.clone());
                }
            }

            // Determine cardinality (if from_column is unique or primary key, it's 1:1, else 1:N)
            let is_unique_fk = tables
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(&fk.table_name))
                .map(|t| {
                    t.indexes.iter().any(|idx| {
                        idx.is_unique
                            && idx.columns.len() == fk.columns.len()
                            && idx
                                .columns
                                .iter()
                                .all(|c| fk.columns.iter().any(|fc| fc.eq_ignore_ascii_case(c)))
                    })
                })
                .unwrap_or(false);

            let cardinality = if is_unique_fk {
                RelationCardinality::OneToOne
            } else {
                RelationCardinality::OneToMany
            };

            let edge_id = format!(
                "{}_{}_{}",
                fk.table_name,
                fk.referenced_table,
                fk.columns.join("_")
            );

            relations.push(RelationEdge {
                id: edge_id,
                constraint_name: fk.name.clone(),
                from_table: fk.table_name.clone(),
                from_columns: fk.columns.clone(),
                to_table: fk.referenced_table.clone(),
                to_columns: fk.referenced_columns.clone(),
                cardinality,
                on_update: fk.on_update.clone(),
                on_delete: fk.on_delete.clone(),
            });
        }

        // 2. Update dimensions for all nodes
        for tbl in &mut tables {
            tbl.update_dimensions();
        }

        let mut graph = Self {
            tables,
            relations,
            canvas_width: 800.0,
            canvas_height: 600.0,
        };

        // 3. Compute 2D auto-layout
        graph.compute_auto_layout();
        graph
    }

    /// Automatically computes (x, y) coordinates for all tables in a clean, legible arrangement.
    pub fn compute_auto_layout(&mut self) {
        if self.tables.is_empty() {
            self.canvas_width = 800.0;
            self.canvas_height = 600.0;
            return;
        }

        const MARGIN_LEFT: f32 = 40.0;
        const MARGIN_TOP: f32 = 40.0;
        const H_SPACING: f32 = 70.0;
        const V_SPACING: f32 = 40.0;
        const COLUMNS_PER_ROW: usize = 3;

        // Determine ordering: place tables with the most relationships first (hubs)
        let mut degree_map: BTreeMap<String, usize> = BTreeMap::new();
        for rel in &self.relations {
            *degree_map.entry(rel.from_table.clone()).or_insert(0) += 1;
            *degree_map.entry(rel.to_table.clone()).or_insert(0) += 1;
        }

        // Sort tables by connection degree descending, then by name ascending
        self.tables.sort_by(|a, b| {
            let deg_a = degree_map.get(&a.name).copied().unwrap_or(0);
            let deg_b = degree_map.get(&b.name).copied().unwrap_or(0);
            deg_b.cmp(&deg_a).then_with(|| a.name.cmp(&b.name))
        });

        // Track column X positions and column current bottom Y
        let num_cols = COLUMNS_PER_ROW.min(self.tables.len()).max(1);
        let mut col_bottom_y = vec![MARGIN_TOP; num_cols];

        let mut max_x: f32 = 800.0;
        let mut max_y: f32 = 600.0;

        for (idx, tbl) in self.tables.iter_mut().enumerate() {
            let col_idx = idx % num_cols;
            let x = MARGIN_LEFT + (col_idx as f32 * (tbl.width + H_SPACING));
            let y = col_bottom_y[col_idx];

            tbl.x = x;
            tbl.y = y;

            col_bottom_y[col_idx] += tbl.height + V_SPACING;

            let right = x + tbl.width + MARGIN_LEFT;
            let bottom = y + tbl.height + MARGIN_TOP;
            if right > max_x {
                max_x = right;
            }
            if bottom > max_y {
                max_y = bottom;
            }
        }

        self.canvas_width = max_x + 60.0;
        self.canvas_height = max_y + 60.0;
    }

    /// Find table node by name.
    pub fn find_table(&self, name: &str) -> Option<&TableNode> {
        self.tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Retrieve all tables directly connected to the specified table.
    pub fn connected_tables(&self, table_name: &str) -> HashSet<String> {
        let mut set = HashSet::new();
        for rel in &self.relations {
            if rel.from_table.eq_ignore_ascii_case(table_name) {
                set.insert(rel.to_table.clone());
            } else if rel.to_table.eq_ignore_ascii_case(table_name) {
                set.insert(rel.from_table.clone());
            }
        }
        set
    }

    /// Retrieve all relation edges involving the specified table.
    pub fn connected_relations(&self, table_name: &str) -> Vec<&RelationEdge> {
        self.relations
            .iter()
            .filter(|r| {
                r.from_table.eq_ignore_ascii_case(table_name)
                    || r.to_table.eq_ignore_ascii_case(table_name)
            })
            .collect()
    }

    /// Exports the current ER diagram graph as standard Mermaid Markdown syntax.
    pub fn to_mermaid(&self) -> String {
        let mut lines = Vec::new();
        lines.push("erDiagram".to_string());

        // 1. Render Table Definitions
        for tbl in &self.tables {
            let safe_table_name = sanitize_mermaid_ident(&tbl.name);
            lines.push(format!("    {} {{", safe_table_name));

            for col in &tbl.columns {
                let sanitized_type = sanitize_mermaid_type(&col.data_type);
                let sanitized_col = sanitize_mermaid_ident(&col.name);

                let key_tag = if col.is_primary_key {
                    "PK"
                } else if tbl.is_foreign_key(&col.name) {
                    "FK"
                } else {
                    ""
                };

                let comment = col.description.as_deref().unwrap_or("");
                let comment_tag = if !comment.is_empty() {
                    format!(" \"{}\"", comment.replace('"', "'"))
                } else {
                    String::new()
                };

                if key_tag.is_empty() {
                    lines.push(format!(
                        "        {} {}{}",
                        sanitized_type, sanitized_col, comment_tag
                    ));
                } else {
                    lines.push(format!(
                        "        {} {} {}{}",
                        sanitized_type, sanitized_col, key_tag, comment_tag
                    ));
                }
            }

            lines.push("    }".to_string());
        }

        // 2. Render Relationships
        for rel in &self.relations {
            let from_safe = sanitize_mermaid_ident(&rel.from_table);
            let to_safe = sanitize_mermaid_ident(&rel.to_table);
            let label = rel.from_columns.join("_");

            lines.push(format!(
                "    {} {} {} : \"{}\"",
                to_safe,
                rel.cardinality.mermaid_arrow(),
                from_safe,
                label
            ));
        }

        lines.join("\n")
    }
}

fn sanitize_mermaid_ident(ident: &str) -> String {
    let s: String = ident
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "entity".to_string()
    } else {
        s
    }
}

fn sanitize_mermaid_type(data_type: &str) -> String {
    let cleaned: String = data_type
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if cleaned.is_empty() {
        "string".to_string()
    } else {
        cleaned.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_col(name: &str, data_type: &str, pk: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            is_nullable: false,
            is_primary_key: pk,
            is_auto_increment: false,
            default_value: None,
            description: None,
        }
    }

    #[test]
    fn test_er_diagram_graph_construction() {
        let mut users = TableNode::new("users", Some("public"), false);
        users.columns = vec![
            mock_col("id", "integer", true),
            mock_col("email", "varchar(255)", false),
        ];

        let mut orders = TableNode::new("orders", Some("public"), false);
        orders.columns = vec![
            mock_col("id", "integer", true),
            mock_col("user_id", "integer", false),
            mock_col("total", "numeric", false),
        ];

        let fks = vec![ForeignKeyInfo {
            name: "fk_orders_user".to_string(),
            table_name: "orders".to_string(),
            columns: vec!["user_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_update: Some("CASCADE".to_string()),
            on_delete: Some("SET NULL".to_string()),
        }];

        let graph = ErDiagramGraph::from_metadata(vec![users, orders], &fks);

        assert_eq!(graph.tables.len(), 2);
        assert_eq!(graph.relations.len(), 1);

        let rel = &graph.relations[0];
        assert_eq!(rel.from_table, "orders");
        assert_eq!(rel.to_table, "users");
        assert_eq!(rel.from_columns, vec!["user_id"]);
        assert_eq!(rel.to_columns, vec!["id"]);
        assert_eq!(rel.cardinality, RelationCardinality::OneToMany);

        let connected = graph.connected_tables("users");
        assert!(connected.contains("orders"));

        let orders_connected = graph.connected_tables("orders");
        assert!(orders_connected.contains("users"));
    }

    #[test]
    fn test_mermaid_export() {
        let mut users = TableNode::new("users", None, false);
        users.columns = vec![
            mock_col("id", "integer", true),
            mock_col("username", "text", false),
        ];

        let mut posts = TableNode::new("posts", None, false);
        posts.columns = vec![
            mock_col("id", "integer", true),
            mock_col("author_id", "integer", false),
        ];

        let fks = vec![ForeignKeyInfo {
            name: "fk_posts_author".to_string(),
            table_name: "posts".to_string(),
            columns: vec!["author_id".to_string()],
            referenced_table: "users".to_string(),
            referenced_columns: vec!["id".to_string()],
            on_update: None,
            on_delete: None,
        }];

        let graph = ErDiagramGraph::from_metadata(vec![users, posts], &fks);
        let mermaid = graph.to_mermaid();

        assert!(mermaid.starts_with("erDiagram"));
        assert!(mermaid.contains("users {"));
        assert!(mermaid.contains("posts {"));
        assert!(mermaid.contains("integer id PK"));
        assert!(mermaid.contains("users ||--o{ posts : \"author_id\""));
    }

    #[test]
    fn test_auto_layout_bounds() {
        let mut tables = Vec::new();
        for i in 1..=5 {
            let mut tbl = TableNode::new(&format!("table_{i}"), None, false);
            tbl.columns = vec![mock_col("id", "int", true)];
            tbl.update_dimensions();
            tables.push(tbl);
        }

        let graph = ErDiagramGraph::from_metadata(tables, &[]);
        assert!(graph.canvas_width > 0.0);
        assert!(graph.canvas_height > 0.0);

        // Check that tables do not share the exact same (x, y) coordinates
        let mut coords = HashSet::new();
        for tbl in &graph.tables {
            let key = (tbl.x as i32, tbl.y as i32);
            assert!(
                coords.insert(key),
                "Overlapping table coordinates detected at {:?}",
                key
            );
        }
    }
}
