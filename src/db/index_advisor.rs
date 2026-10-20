//! Database Index Advisor & Health Diagnostics Engine.
//!
//! Provides static schema audit, redundant index detection, left-prefix coverage analysis,
//! unindexed foreign key identification, and query-driven missing index recommendations.

use crate::db::types::{ColumnInfo, DatabaseFamily, ForeignKeyInfo, IndexInfo, quote_ident};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Severity rating for an index health issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexIssueSeverity {
    Info,
    Warning,
    Critical,
}

impl IndexIssueSeverity {
    pub fn badge_label(&self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::Warning => "WARNING",
            Self::Info => "INFO",
        }
    }
}

/// Category classification of index anomalies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexIssueType {
    /// Two or more indexes have the exact same columns in the exact same order.
    ExactDuplicate { duplicate_of: String },
    /// The index's columns form a strict left-prefix of another wider composite index.
    PrefixCovered {
        covered_by: String,
        covering_columns: Vec<String>,
    },
    /// A foreign key constraint has no corresponding index on its referencing columns.
    UnindexedForeignKey {
        foreign_key_name: String,
        fk_columns: Vec<String>,
        referenced_table: String,
    },
    /// Excessive index count on a single table exceeding optimal write performance thresholds.
    HighIndexCount { count: usize },
}

/// A specific diagnostic issue identified on a table's index structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexHealthIssue {
    pub id: String,
    pub severity: IndexIssueSeverity,
    pub issue_type: IndexIssueType,
    pub table_name: String,
    pub index_name: Option<String>,
    pub affected_columns: Vec<String>,
    pub title: String,
    pub description: String,
    pub impact: String,
    pub remediation_sql: Option<String>,
}

/// Recommended index definition derived from slow queries, filters, or foreign keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingIndexRecommendation {
    pub id: String,
    pub table_name: String,
    pub suggested_index_name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub reason: String,
    pub estimated_improvement: String,
    pub create_ddl: String,
}

/// Index health quality tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexHealthScoreLevel {
    Excellent, // 90-100
    Good,      // 75-89
    Fair,      // 60-74
    Poor,      // 0-59
}

impl IndexHealthScoreLevel {
    pub fn from_score(score: u32) -> Self {
        match score {
            90..=100 => Self::Excellent,
            75..=89 => Self::Good,
            60..=74 => Self::Fair,
            _ => Self::Poor,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Excellent => "Excellent (优秀)",
            Self::Good => "Good (良好)",
            Self::Fair => "Fair (一般)",
            Self::Poor => "Poor (需优化)",
        }
    }
}

/// Comprehensive health audit report for a table's indexing state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexHealthReport {
    pub table_name: String,
    pub health_score: u32,
    pub score_level: IndexHealthScoreLevel,
    pub total_indexes: usize,
    pub existing_indexes: Vec<IndexInfo>,
    pub issues: Vec<IndexHealthIssue>,
    pub missing_recommendations: Vec<MissingIndexRecommendation>,
    pub generated_at: DateTime<Utc>,
}

/// Formats a CREATE INDEX statement appropriate for the target database family.
pub fn render_create_index_ddl(
    index_name: &str,
    table_name: &str,
    columns: &[String],
    is_unique: bool,
    family: DatabaseFamily,
) -> String {
    let quoted_table = quote_ident(table_name, family);
    let quoted_index = quote_ident(index_name, family);
    let cols_str = columns
        .iter()
        .map(|c| quote_ident(c, family))
        .collect::<Vec<_>>()
        .join(", ");

    let unique_str = if is_unique { "UNIQUE " } else { "" };

    match family {
        DatabaseFamily::Postgres => {
            format!(
                "CREATE {unique_str}INDEX CONCURRENTLY IF NOT EXISTS {quoted_index} ON {quoted_table} ({cols_str});"
            )
        }
        DatabaseFamily::MySql => {
            format!("CREATE {unique_str}INDEX {quoted_index} ON {quoted_table} ({cols_str});")
        }
        DatabaseFamily::Sqlite => {
            format!(
                "CREATE {unique_str}INDEX IF NOT EXISTS {quoted_index} ON {quoted_table} ({cols_str});"
            )
        }
    }
}

/// Formats a DROP INDEX statement appropriate for the target database family.
pub fn render_drop_index_ddl(index_name: &str, table_name: &str, family: DatabaseFamily) -> String {
    let quoted_index = quote_ident(index_name, family);
    let quoted_table = quote_ident(table_name, family);

    match family {
        DatabaseFamily::Postgres => {
            format!("DROP INDEX CONCURRENTLY IF EXISTS {quoted_index};")
        }
        DatabaseFamily::MySql => {
            format!("DROP INDEX {quoted_index} ON {quoted_table};")
        }
        DatabaseFamily::Sqlite => {
            format!("DROP INDEX IF EXISTS {quoted_index};")
        }
    }
}

/// Sanitizes column names to produce clean, safe identifiers for suggested indexes.
fn generate_index_identifier(table_name: &str, columns: &[String]) -> String {
    let cols_suffix = columns
        .iter()
        .map(|c| c.to_lowercase().replace(' ', "_"))
        .collect::<Vec<_>>()
        .join("_");
    format!("idx_{table_name}_{cols_suffix}")
}

/// Perform a comprehensive index health audit on a specific table.
pub fn audit_table_indexes(
    table_name: &str,
    indexes: &[IndexInfo],
    foreign_keys: &[ForeignKeyInfo],
    family: DatabaseFamily,
) -> IndexHealthReport {
    let mut issues = Vec::new();
    let total_indexes = indexes.len();

    // 1. Audit for Exact Duplicate Indexes
    for i in 0..indexes.len() {
        for j in (i + 1)..indexes.len() {
            let idx_a = &indexes[i];
            let idx_b = &indexes[j];

            // Ignore primary keys as standard base indexes
            if idx_a.is_primary || idx_b.is_primary {
                continue;
            }

            if idx_a.columns == idx_b.columns && idx_a.is_unique == idx_b.is_unique {
                let drop_sql = render_drop_index_ddl(&idx_b.name, table_name, family);
                issues.push(IndexHealthIssue {
                    id: uuid::Uuid::new_v4().to_string(),
                    severity: IndexIssueSeverity::Critical,
                    issue_type: IndexIssueType::ExactDuplicate {
                        duplicate_of: idx_a.name.clone(),
                    },
                    table_name: table_name.to_string(),
                    index_name: Some(idx_b.name.clone()),
                    affected_columns: idx_b.columns.clone(),
                    title: format!("Exact duplicate of index '{}'", idx_a.name),
                    description: format!(
                        "Index '{}' has the exact same columns ({}) and uniqueness as '{}'.",
                        idx_b.name,
                        idx_b.columns.join(", "),
                        idx_a.name
                    ),
                    impact: "Wastes storage space and incurs 100% redundant write overhead on every INSERT/UPDATE/DELETE.".into(),
                    remediation_sql: Some(drop_sql),
                });
            }
        }
    }

    // 2. Audit for Left-Prefix Covered Indexes
    for (i, idx_narrow) in indexes.iter().enumerate() {
        if idx_narrow.is_primary || idx_narrow.is_unique {
            // A unique index cannot be safely dropped even if covered, because it enforces uniqueness
            continue;
        }

        for (j, idx_wide) in indexes.iter().enumerate() {
            if i == j {
                continue;
            }

            // If idx_wide starts with idx_narrow's columns and is strictly longer
            if idx_wide.columns.len() > idx_narrow.columns.len()
                && idx_wide.columns.starts_with(&idx_narrow.columns)
            {
                let drop_sql = render_drop_index_ddl(&idx_narrow.name, table_name, family);
                issues.push(IndexHealthIssue {
                    id: uuid::Uuid::new_v4().to_string(),
                    severity: IndexIssueSeverity::Warning,
                    issue_type: IndexIssueType::PrefixCovered {
                        covered_by: idx_wide.name.clone(),
                        covering_columns: idx_wide.columns.clone(),
                    },
                    table_name: table_name.to_string(),
                    index_name: Some(idx_narrow.name.clone()),
                    affected_columns: idx_narrow.columns.clone(),
                    title: format!("Redundant index covered by '{}'", idx_wide.name),
                    description: format!(
                        "Index '{}' ({}) is fully covered by the left-prefix of composite index '{}' ({}).",
                        idx_narrow.name,
                        idx_narrow.columns.join(", "),
                        idx_wide.name,
                        idx_wide.columns.join(", ")
                    ),
                    impact: "B-Tree lookups on these columns can be satisfied by the wider index. Dropping this eliminates write overhead.".into(),
                    remediation_sql: Some(drop_sql),
                });
                break; // One covering index is sufficient to flag redundancy
            }
        }
    }

    // 3. Audit Foreign Keys for Missing Indexes
    for fk in foreign_keys {
        if !fk.table_name.eq_ignore_ascii_case(table_name) {
            continue;
        }

        let is_indexed = indexes.iter().any(|idx| {
            // An index satisfies a foreign key if its columns start with the FK columns
            idx.columns.starts_with(&fk.columns)
        });

        if !is_indexed {
            let suggested_name = generate_index_identifier(table_name, &fk.columns);
            let create_sql =
                render_create_index_ddl(&suggested_name, table_name, &fk.columns, false, family);

            issues.push(IndexHealthIssue {
                id: uuid::Uuid::new_v4().to_string(),
                severity: IndexIssueSeverity::Critical,
                issue_type: IndexIssueType::UnindexedForeignKey {
                    foreign_key_name: fk.name.clone(),
                    fk_columns: fk.columns.clone(),
                    referenced_table: fk.referenced_table.clone(),
                },
                table_name: table_name.to_string(),
                index_name: None,
                affected_columns: fk.columns.clone(),
                title: format!("Unindexed Foreign Key '{}'", fk.name),
                description: format!(
                    "Foreign key '{}' references '{}({})' on columns ({}) without a supporting index.",
                    fk.name,
                    fk.referenced_table,
                    fk.referenced_columns.join(", "),
                    fk.columns.join(", ")
                ),
                impact: "Parent table DELETE or UPDATE operations may cause full table scans or extensive table locks on this child table.".into(),
                remediation_sql: Some(create_sql),
            });
        }
    }

    // 4. Audit Table Index Saturation (High Index Count)
    if total_indexes > 7 {
        issues.push(IndexHealthIssue {
            id: uuid::Uuid::new_v4().to_string(),
            severity: IndexIssueSeverity::Info,
            issue_type: IndexIssueType::HighIndexCount { count: total_indexes },
            table_name: table_name.to_string(),
            index_name: None,
            affected_columns: Vec::new(),
            title: format!("High Index Count ({total_indexes} indexes)"),
            description: format!(
                "Table has {total_indexes} indexes. Having more than 7 indexes on a table can significantly degrade INSERT and UPDATE throughput.",
            ),
            impact: "Review whether all indexes are actively utilized by core application queries.".into(),
            remediation_sql: None,
        });
    }

    // Calculate score: Start with 100, deduct penalties based on severity
    let mut penalty = 0;
    for issue in &issues {
        match issue.severity {
            IndexIssueSeverity::Critical => penalty += 25,
            IndexIssueSeverity::Warning => penalty += 12,
            IndexIssueSeverity::Info => penalty += 4,
        }
    }

    let health_score = 100u32.saturating_sub(penalty);
    let score_level = IndexHealthScoreLevel::from_score(health_score);

    IndexHealthReport {
        table_name: table_name.to_string(),
        health_score,
        score_level,
        total_indexes,
        existing_indexes: indexes.to_vec(),
        issues,
        missing_recommendations: Vec::new(),
        generated_at: Utc::now(),
    }
}

/// Analyze a SQL query and recommend missing indexes based on WHERE, JOIN, and ORDER BY clauses.
pub fn recommend_indexes_for_query(
    sql: &str,
    table_name: &str,
    table_columns: &[ColumnInfo],
    existing_indexes: &[IndexInfo],
    family: DatabaseFamily,
) -> Vec<MissingIndexRecommendation> {
    let mut recommendations = Vec::new();
    let lower_sql = sql.to_lowercase();

    // Check if the query references the table
    let table_lower = table_name.to_lowercase();
    if !lower_sql.contains(&table_lower) {
        return recommendations;
    }

    let col_map: std::collections::HashMap<String, String> = table_columns
        .iter()
        .map(|c| (c.name.to_lowercase(), c.name.clone()))
        .collect();

    // 1. Extract potential equality and range filter columns from WHERE clause
    let mut equality_cols = Vec::new();
    let mut range_cols = Vec::new();
    let mut order_cols = Vec::new();

    if let Some(where_pos) = lower_sql.find("where") {
        let after_where = &lower_sql[where_pos + 5..];
        let end_pos = after_where
            .find("order by")
            .or_else(|| after_where.find("group by"))
            .or_else(|| after_where.find("limit"))
            .unwrap_or(after_where.len());
        let where_clause = &after_where[..end_pos];

        // Tokenize conditions separated by AND / OR
        for part in where_clause.split(" and ") {
            let part_trimmed = part.trim();
            // Match col = ...
            if let Some((lhs, _)) = part_trimmed.split_once('=') {
                let col_raw = extract_column_name(lhs.trim());
                if let Some(real_col) = col_map.get(&col_raw.to_lowercase()) {
                    if !equality_cols.contains(real_col) {
                        equality_cols.push(real_col.clone());
                    }
                }
            } else if part_trimmed.contains('>')
                || part_trimmed.contains('<')
                || part_trimmed.contains(" between ")
                || part_trimmed.contains(" like ")
            {
                // Range match
                let tokens: Vec<&str> = part_trimmed
                    .split(&['>', '<', ' '][..])
                    .filter(|s| !s.is_empty())
                    .collect();
                if let Some(first) = tokens.first() {
                    let col_raw = extract_column_name(first.trim());
                    if let Some(real_col) = col_map.get(&col_raw.to_lowercase()) {
                        if !equality_cols.contains(real_col) && !range_cols.contains(real_col) {
                            range_cols.push(real_col.clone());
                        }
                    }
                }
            }
        }
    }

    // 2. Extract ORDER BY columns
    if let Some(order_pos) = lower_sql.find("order by") {
        let after_order = &lower_sql[order_pos + 8..];
        let end_pos = after_order.find("limit").unwrap_or(after_order.len());
        let order_clause = &after_order[..end_pos];

        for part in order_clause.split(',') {
            let clean = part.trim();
            let tokens: Vec<&str> = clean.split_whitespace().collect();
            if let Some(col_token) = tokens.first() {
                let col_raw = extract_column_name(col_token);
                if let Some(real_col) = col_map.get(&col_raw.to_lowercase()) {
                    if !equality_cols.contains(real_col)
                        && !range_cols.contains(real_col)
                        && !order_cols.contains(real_col)
                    {
                        order_cols.push(real_col.clone());
                    }
                }
            }
        }
    }

    // Combine in standard indexing best-practice order:
    // [Equality columns] -> [Range columns] -> [Sort columns]
    let mut candidate_cols = Vec::new();
    for col in equality_cols {
        candidate_cols.push(col);
    }
    for col in range_cols {
        if !candidate_cols.contains(&col) {
            candidate_cols.push(col);
        }
    }
    for col in order_cols {
        if !candidate_cols.contains(&col) {
            candidate_cols.push(col);
        }
    }

    if candidate_cols.is_empty() {
        return recommendations;
    }

    // Check if any existing index already covers this candidate prefix
    let already_covered = existing_indexes.iter().any(|idx| {
        idx.columns.starts_with(&candidate_cols)
            || (candidate_cols.len() == 1 && idx.columns.first() == candidate_cols.first())
    });

    if !already_covered {
        let suggested_name = generate_index_identifier(table_name, &candidate_cols);
        let ddl =
            render_create_index_ddl(&suggested_name, table_name, &candidate_cols, false, family);

        let improvement = if candidate_cols.len() > 1 {
            "Converts multi-column filter and sort into an efficient composite index range scan."
                .to_string()
        } else {
            "Avoids full table scan by adding a high-selectivity lookup index.".to_string()
        };

        recommendations.push(MissingIndexRecommendation {
            id: uuid::Uuid::new_v4().to_string(),
            table_name: table_name.to_string(),
            suggested_index_name: suggested_name,
            columns: candidate_cols.clone(),
            is_unique: false,
            reason: format!(
                "Query filters or sorts on ({}) without a matching index.",
                candidate_cols.join(", ")
            ),
            estimated_improvement: improvement,
            create_ddl: ddl,
        });
    }

    recommendations
}

/// Helper to strip table/alias prefixes (e.g. `u.id` -> `id`, `"users"."name"` -> `name`).
fn extract_column_name(raw: &str) -> String {
    let unquoted = raw.trim().trim_matches(&['"', '`', '\'', ';'][..]);
    if let Some((_, col)) = unquoted.rsplit_once('.') {
        col.trim_matches(&['"', '`', '\''][..]).to_string()
    } else {
        unquoted.to_string()
    }
}

/// Produce a unified SQL remediation script for all issues in an index health report.
pub fn generate_all_remediation_sql(report: &IndexHealthReport, _family: DatabaseFamily) -> String {
    let mut out = String::new();
    out.push_str("-- ========================================================\n");
    out.push_str(&format!(
        "-- zqlcrab Index Optimization Remediation Plan for [{}]\n",
        report.table_name
    ));
    out.push_str(&format!(
        "-- Generated At: {}\n",
        report.generated_at.to_rfc3339()
    ));
    out.push_str(&format!(
        "-- Health Score: {}/100 ({})\n",
        report.health_score,
        report.score_level.display_name()
    ));
    out.push_str(&format!(
        "-- Issues Detected: {}\n",
        report.issues.len() + report.missing_recommendations.len()
    ));
    out.push_str("-- ========================================================\n\n");

    let mut drop_statements = Vec::new();
    let mut create_statements = Vec::new();

    for issue in &report.issues {
        if let Some(sql) = &issue.remediation_sql {
            if sql.starts_with("DROP") {
                drop_statements.push((issue, sql));
            } else if sql.starts_with("CREATE") {
                create_statements.push((issue, sql));
            }
        }
    }

    if !drop_statements.is_empty() {
        out.push_str("-- --------------------------------------------------------\n");
        out.push_str("-- 1. Drop Redundant & Duplicate Indexes (Saves Write/Disk Overhead)\n");
        out.push_str("-- --------------------------------------------------------\n");
        for (issue, sql) in drop_statements {
            out.push_str(&format!("-- Issue: {}\n", issue.title));
            out.push_str(&format!("-- Impact: {}\n", issue.impact));
            out.push_str(sql);
            out.push_str("\n\n");
        }
    }

    if !create_statements.is_empty() || !report.missing_recommendations.is_empty() {
        out.push_str("-- --------------------------------------------------------\n");
        out.push_str("-- 2. Create Missing & Supporting Indexes (Accelerates Queries)\n");
        out.push_str("-- --------------------------------------------------------\n");
        for (issue, sql) in create_statements {
            out.push_str(&format!("-- Issue: {}\n", issue.title));
            out.push_str(&format!("-- Target: {}\n", issue.impact));
            out.push_str(sql);
            out.push_str("\n\n");
        }

        for rec in &report.missing_recommendations {
            out.push_str(&format!(
                "-- Recommendation: {}\n",
                rec.suggested_index_name
            ));
            out.push_str(&format!("-- Reason: {}\n", rec.reason));
            out.push_str(&rec.create_ddl);
            out.push_str("\n\n");
        }
    }

    if report.issues.is_empty() && report.missing_recommendations.is_empty() {
        out.push_str("-- Table index structure is healthy! No actions needed.\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_index(
        name: &str,
        table: &str,
        cols: &[&str],
        is_unique: bool,
        is_pk: bool,
    ) -> IndexInfo {
        IndexInfo {
            name: name.to_string(),
            table_name: table.to_string(),
            columns: cols.iter().map(|s| s.to_string()).collect(),
            is_unique,
            is_primary: is_pk,
        }
    }

    fn make_fk(
        name: &str,
        table: &str,
        cols: &[&str],
        ref_table: &str,
        ref_cols: &[&str],
    ) -> ForeignKeyInfo {
        ForeignKeyInfo {
            name: name.to_string(),
            table_name: table.to_string(),
            columns: cols.iter().map(|s| s.to_string()).collect(),
            referenced_table: ref_table.to_string(),
            referenced_columns: ref_cols.iter().map(|s| s.to_string()).collect(),
            on_update: None,
            on_delete: None,
        }
    }

    #[test]
    fn test_exact_duplicate_index_detection() {
        let indexes = vec![
            make_index("pk_users", "users", &["id"], true, true),
            make_index("idx_email_1", "users", &["email"], false, false),
            make_index("idx_email_2", "users", &["email"], false, false),
        ];

        let report = audit_table_indexes("users", &indexes, &[], DatabaseFamily::Postgres);
        assert_eq!(report.issues.len(), 1);
        let issue = &report.issues[0];
        assert_eq!(issue.severity, IndexIssueSeverity::Critical);
        assert!(matches!(
            issue.issue_type,
            IndexIssueType::ExactDuplicate { .. }
        ));
        assert_eq!(issue.index_name.as_deref(), Some("idx_email_2"));
        assert!(
            issue
                .remediation_sql
                .as_ref()
                .unwrap()
                .contains("DROP INDEX CONCURRENTLY IF EXISTS")
        );
    }

    #[test]
    fn test_prefix_covered_redundant_index_detection() {
        let indexes = vec![
            make_index("pk_orders", "orders", &["id"], true, true),
            make_index(
                "idx_user_created",
                "orders",
                &["user_id", "created_at"],
                false,
                false,
            ),
            make_index("idx_user_only", "orders", &["user_id"], false, false), // covered by idx_user_created
        ];

        let report = audit_table_indexes("orders", &indexes, &[], DatabaseFamily::MySql);
        assert_eq!(report.issues.len(), 1);
        let issue = &report.issues[0];
        assert_eq!(issue.severity, IndexIssueSeverity::Warning);
        if let IndexIssueType::PrefixCovered {
            covered_by,
            covering_columns,
        } = &issue.issue_type
        {
            assert_eq!(covered_by, "idx_user_created");
            assert_eq!(covering_columns, &["user_id", "created_at"]);
        } else {
            panic!("Expected PrefixCovered issue");
        }
        assert!(
            issue
                .remediation_sql
                .as_ref()
                .unwrap()
                .contains("DROP INDEX `idx_user_only` ON `orders`")
        );
    }

    #[test]
    fn test_unindexed_foreign_key_detection() {
        let indexes = vec![make_index(
            "pk_order_items",
            "order_items",
            &["id"],
            true,
            true,
        )];

        let fks = vec![
            make_fk(
                "fk_items_order",
                "order_items",
                &["order_id"],
                "orders",
                &["id"],
            ),
            make_fk(
                "fk_items_product",
                "order_items",
                &["product_id"],
                "products",
                &["id"],
            ),
        ];

        let report = audit_table_indexes("order_items", &indexes, &fks, DatabaseFamily::Sqlite);
        assert_eq!(report.issues.len(), 2);
        assert_eq!(report.issues[0].severity, IndexIssueSeverity::Critical);
        assert!(report.issues[0].title.contains("fk_items_order"));
        assert!(
            report.issues[0]
                .remediation_sql
                .as_ref()
                .unwrap()
                .contains("CREATE INDEX IF NOT EXISTS")
        );
    }

    #[test]
    fn test_recommend_indexes_for_query() {
        let cols = vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "bigint".to_string(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: true,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "tenant_id".to_string(),
                data_type: "int".to_string(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "status".to_string(),
                data_type: "varchar".to_string(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "created_at".to_string(),
                data_type: "timestamp".to_string(),
                is_nullable: false,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];

        let indexes = vec![make_index("pk_invoices", "invoices", &["id"], true, true)];

        let query = "SELECT * FROM invoices WHERE tenant_id = 42 AND status = 'active' ORDER BY created_at DESC LIMIT 50";
        let recs = recommend_indexes_for_query(
            query,
            "invoices",
            &cols,
            &indexes,
            DatabaseFamily::Postgres,
        );

        assert_eq!(recs.len(), 1);
        let rec = &recs[0];
        assert_eq!(rec.table_name, "invoices");
        // Equality columns (tenant_id, status) first, sort column (created_at) second
        assert!(rec.columns.contains(&"tenant_id".to_string()));
        assert!(rec.columns.contains(&"status".to_string()));
        assert!(rec.columns.contains(&"created_at".to_string()));
        assert!(
            rec.create_ddl
                .contains("CREATE INDEX CONCURRENTLY IF NOT EXISTS")
        );
    }

    #[test]
    fn test_generate_all_remediation_sql() {
        let indexes = vec![
            make_index("pk_t", "t", &["id"], true, true),
            make_index("idx_dup1", "t", &["code"], false, false),
            make_index("idx_dup2", "t", &["code"], false, false),
        ];
        let report = audit_table_indexes("t", &indexes, &[], DatabaseFamily::Postgres);
        let sql = generate_all_remediation_sql(&report, DatabaseFamily::Postgres);

        assert!(sql.contains("zqlcrab Index Optimization Remediation Plan"));
        assert!(sql.contains("DROP INDEX CONCURRENTLY IF EXISTS"));
    }
}
