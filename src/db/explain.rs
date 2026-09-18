//! EXPLAIN wrapping and plan parsing for Postgres, MySQL, and SQLite.

use crate::db::safety::QuerySafetyValidator;
use crate::db::types::{DatabaseFamily, QueryResult, QueryValue};
use serde_json::Value as JsonValue;

/// One node in a query plan tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplainNode {
    pub node_type: String,
    pub relation: Option<String>,
    pub alias: Option<String>,
    pub index: Option<String>,
    pub startup_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub plan_rows: Option<f64>,
    pub details: String,
    pub children: Vec<ExplainNode>,
}

impl ExplainNode {
    pub fn cost_label(&self) -> Option<String> {
        match (self.startup_cost, self.total_cost) {
            (Some(start), Some(total)) => Some(format!("c:{start:.2}..{total:.2}")),
            (None, Some(total)) => Some(format!("c:{total:.2}")),
            (Some(start), None) => Some(format!("c:{start:.2}")),
            (None, None) => None,
        }
    }

    pub fn rows_label(&self) -> Option<String> {
        self.plan_rows.map(|rows| {
            if rows.fract() == 0.0 {
                format!("e:{}", rows as i64)
            } else {
                format!("e:{rows:.2}")
            }
        })
    }

    pub fn display_name(&self) -> String {
        match self.relation.as_deref() {
            Some(rel) if !self.node_type.to_ascii_lowercase().contains(rel) => {
                format!("{} on {}", self.node_type, rel)
            }
            _ => self.node_type.clone(),
        }
    }
}

/// Parsed EXPLAIN output plus the original raw text.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplainPlan {
    pub dialect: DatabaseFamily,
    pub raw: String,
    pub roots: Vec<ExplainNode>,
}

impl ExplainPlan {
    pub fn node_count(&self) -> usize {
        fn count(nodes: &[ExplainNode]) -> usize {
            nodes.iter().map(|n| 1 + count(&n.children)).sum()
        }
        count(&self.roots)
    }

    pub fn estimated_total_cost(&self) -> Option<f64> {
        self.roots.iter().filter_map(|n| n.total_cost).reduce(f64::max)
    }

    pub fn most_expensive(&self) -> Option<&ExplainNode> {
        fn walk<'a>(nodes: &'a [ExplainNode], best: &mut Option<&'a ExplainNode>) {
            for node in nodes {
                let better = match (*best, node.total_cost) {
                    (None, Some(_)) => true,
                    (Some(current), Some(cost)) => cost > current.total_cost.unwrap_or(f64::MIN),
                    _ => false,
                };
                if better {
                    *best = Some(node);
                }
                walk(&node.children, best);
            }
        }
        let mut best = None;
        walk(&self.roots, &mut best);
        best
    }

    pub fn flatten(&self) -> Vec<(usize, &ExplainNode)> {
        fn walk<'a>(nodes: &'a [ExplainNode], depth: usize, out: &mut Vec<(usize, &'a ExplainNode)>) {
            for node in nodes {
                out.push((depth, node));
                walk(&node.children, depth + 1, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.roots, 0, &mut out);
        out
    }

    pub fn dialect_label(&self) -> &'static str {
        match self.dialect {
            DatabaseFamily::Postgres => "POSTGRESQL",
            DatabaseFamily::MySql => "MYSQL",
            DatabaseFamily::Sqlite => "SQLITE",
        }
    }
}

/// Prefix SQL with a dialect-appropriate EXPLAIN statement.
pub fn wrap_explain_sql(sql: &str, family: DatabaseFamily) -> String {
    let cleaned = QuerySafetyValidator::clean_sql(sql);
    let body = cleaned.trim_end_matches(';').trim();
    if body.is_empty() {
        return String::new();
    }

    let first = body
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if first == "EXPLAIN" {
        return body.to_string();
    }

    match family {
        DatabaseFamily::Postgres => format!("EXPLAIN (FORMAT JSON) {body}"),
        DatabaseFamily::MySql => format!("EXPLAIN FORMAT=JSON {body}"),
        DatabaseFamily::Sqlite => format!("EXPLAIN QUERY PLAN {body}"),
    }
}

/// Build an [`ExplainPlan`] from a query result returned by [`wrap_explain_sql`].
pub fn parse_explain_result(family: DatabaseFamily, result: &QueryResult) -> ExplainPlan {
    let raw = result_to_raw(result);
    let roots = match family {
        DatabaseFamily::Sqlite => parse_sqlite_plan(result).unwrap_or_else(|| parse_json_plan(&raw)),
        DatabaseFamily::Postgres | DatabaseFamily::MySql => {
            let from_json = parse_json_plan(&raw);
            if from_json.is_empty() {
                parse_sqlite_plan(result).unwrap_or_default()
            } else {
                from_json
            }
        }
    };

    ExplainPlan {
        dialect: family,
        raw: pretty_raw(&raw),
        roots,
    }
}

fn result_to_raw(result: &QueryResult) -> String {
    if result.rows.len() == 1 && result.columns.len() == 1 {
        return result.rows[0][0].to_display_string();
    }

    let mut lines = Vec::new();
    if !result.columns.is_empty() {
        lines.push(result.columns.join(" | "));
    }
    for row in &result.rows {
        let cells: Vec<String> = row.iter().map(QueryValue::to_display_string).collect();
        lines.push(cells.join(" | "));
    }
    lines.join("\n")
}

fn pretty_raw(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str::<JsonValue>(trimmed) {
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    }
}

fn parse_json_plan(raw: &str) -> Vec<ExplainNode> {
    let Ok(value) = serde_json::from_str::<JsonValue>(raw.trim()) else {
        return Vec::new();
    };
    parse_json_value(&value)
}

fn parse_json_value(value: &JsonValue) -> Vec<ExplainNode> {
    match value {
        JsonValue::Array(items) => {
            let mut nodes = Vec::new();
            for item in items {
                nodes.extend(parse_json_value(item));
            }
            nodes
        }
        JsonValue::Object(map) => {
            if let Some(plan) = map.get("Plan") {
                return vec![parse_postgres_node(plan)];
            }
            if let Some(block) = map.get("query_block") {
                return vec![parse_mysql_block(block)];
            }
            if map.contains_key("Node Type") {
                return vec![parse_postgres_node(value)];
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn parse_postgres_node(value: &JsonValue) -> ExplainNode {
    let obj = value.as_object();
    let get_str = |key: &str| {
        obj.and_then(|m| m.get(key))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };
    let get_f64 = |key: &str| {
        obj.and_then(|m| m.get(key)).and_then(|v| match v {
            JsonValue::Number(n) => n.as_f64(),
            JsonValue::String(s) => s.parse().ok(),
            _ => None,
        })
    };

    let children = obj
        .and_then(|m| m.get("Plans"))
        .and_then(|v| v.as_array())
        .map(|plans| plans.iter().map(parse_postgres_node).collect())
        .unwrap_or_default();

    ExplainNode {
        node_type: get_str("Node Type").unwrap_or_else(|| "Plan".to_string()),
        relation: get_str("Relation Name"),
        alias: get_str("Alias"),
        index: get_str("Index Name"),
        startup_cost: get_f64("Startup Cost"),
        total_cost: get_f64("Total Cost"),
        plan_rows: get_f64("Plan Rows"),
        details: String::new(),
        children,
    }
}

fn parse_mysql_block(value: &JsonValue) -> ExplainNode {
    let cost = value
        .pointer("/cost_info/query_cost")
        .and_then(json_f64);
    let mut children = Vec::new();

    if let Some(table) = value.get("table") {
        children.extend(parse_mysql_table_nodes(table));
    }
    if let Some(nested) = value.get("nested_loop").and_then(|v| v.as_array()) {
        for item in nested {
            if let Some(table) = item.get("table") {
                children.extend(parse_mysql_table_nodes(table));
            }
        }
    }
    for wrapper in ["ordering_operation", "grouping_operation", "duplicates_removal"] {
        if let Some(inner) = value.get(wrapper) {
            children.push(parse_mysql_block(inner));
        }
    }

    ExplainNode {
        node_type: "Query block".to_string(),
        relation: None,
        alias: None,
        index: None,
        startup_cost: Some(0.0),
        total_cost: cost,
        plan_rows: value.get("select_id").and_then(json_f64),
        details: String::new(),
        children,
    }
}

fn parse_mysql_table_nodes(value: &JsonValue) -> Vec<ExplainNode> {
    match value {
        JsonValue::Array(items) => items.iter().flat_map(parse_mysql_table_nodes).collect(),
        JsonValue::Object(map) => {
            let access = map
                .get("access_type")
                .and_then(|v| v.as_str())
                .unwrap_or("ALL");
            let node_type = match access {
                "ALL" => "Seq Scan",
                "index" | "range" | "ref" | "eq_ref" | "const" | "unique" => "Index Scan",
                other => other,
            }
            .to_string();
            let relation = map
                .get("table_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let index = map
                .get("key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let rows = map
                .get("rows_examined_per_scan")
                .or_else(|| map.get("rows"))
                .and_then(json_f64);
            let cost = value.pointer("/cost_info/read_cost").and_then(json_f64);
            vec![ExplainNode {
                node_type,
                relation,
                alias: map
                    .get("table_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                index,
                startup_cost: Some(0.0),
                total_cost: cost,
                plan_rows: rows,
                details: String::new(),
                children: Vec::new(),
            }]
        }
        _ => Vec::new(),
    }
}

fn json_f64(value: &JsonValue) -> Option<f64> {
    match value {
        JsonValue::Number(n) => n.as_f64(),
        JsonValue::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn parse_sqlite_plan(result: &QueryResult) -> Option<Vec<ExplainNode>> {
    if result.rows.is_empty() {
        return None;
    }

    let cols: Vec<String> = result
        .columns
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let id_ix = cols.iter().position(|c| c == "id" || c == "selectid");
    let parent_ix = cols.iter().position(|c| c == "parent" || c == "from");
    let detail_ix = cols.iter().position(|c| c == "detail" || c == "comment" || c.ends_with("detail"));

    if let (Some(id_ix), Some(parent_ix), Some(detail_ix)) = (id_ix, parent_ix, detail_ix) {
        #[derive(Clone)]
        struct RawRow {
            id: i64,
            parent: i64,
            detail: String,
        }
        let rows: Vec<RawRow> = result
            .rows
            .iter()
            .filter_map(|row| {
                Some(RawRow {
                    id: value_i64(row.get(id_ix)?)?,
                    parent: value_i64(row.get(parent_ix)?).unwrap_or(-1),
                    detail: row.get(detail_ix)?.to_display_string(),
                })
            })
            .collect();

        fn build(parent: i64, rows: &[RawRow]) -> Vec<ExplainNode> {
            rows.iter()
                .filter(|r| r.parent == parent)
                .map(|r| {
                    let (node_type, relation) = split_sqlite_detail(&r.detail);
                    ExplainNode {
                        node_type,
                        relation,
                        alias: None,
                        index: None,
                        startup_cost: None,
                        total_cost: None,
                        plan_rows: None,
                        details: r.detail.clone(),
                        children: build(r.id, rows),
                    }
                })
                .collect()
        }

        let roots = build(-1, &rows);
        let roots = if roots.is_empty() { build(0, &rows) } else { roots };
        if roots.is_empty() {
            None
        } else {
            Some(roots)
        }
    } else if result.columns.len() == 1 {
        None
    } else {
        let detail_ix = cols.iter().position(|c| c.contains("detail")).unwrap_or(result.columns.len().saturating_sub(1));
        Some(
            result
                .rows
                .iter()
                .map(|row| {
                    let detail = row
                        .get(detail_ix)
                        .map(QueryValue::to_display_string)
                        .unwrap_or_default();
                    let (node_type, relation) = split_sqlite_detail(&detail);
                    ExplainNode {
                        node_type,
                        relation,
                        alias: None,
                        index: None,
                        startup_cost: None,
                        total_cost: None,
                        plan_rows: None,
                        details: detail,
                        children: Vec::new(),
                    }
                })
                .collect(),
        )
    }
}

fn value_i64(value: &QueryValue) -> Option<i64> {
    match value {
        QueryValue::Int(i) => Some(*i),
        QueryValue::Float(f) => Some(*f as i64),
        QueryValue::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn split_sqlite_detail(detail: &str) -> (String, Option<String>) {
    let trimmed = detail.trim();
    if let Some(rest) = trimmed.strip_prefix("SCAN ") {
        let rel = rest
            .split_whitespace()
            .next()
            .map(|s| s.trim_matches('"').to_string());
        ("Seq Scan".to_string(), rel)
    } else if let Some(rest) = trimmed.strip_prefix("SEARCH ") {
        let rel = rest
            .split_whitespace()
            .next()
            .map(|s| s.trim_matches('"').to_string());
        ("Index Scan".to_string(), rel)
    } else {
        (trimmed.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::QueryResult;

    #[test]
    fn wraps_postgres_select() {
        let sql = wrap_explain_sql("SELECT * FROM t LIMIT 100;", DatabaseFamily::Postgres);
        assert_eq!(sql, "EXPLAIN (FORMAT JSON) SELECT * FROM t LIMIT 100");
    }

    #[test]
    fn does_not_double_wrap() {
        let sql = wrap_explain_sql("EXPLAIN SELECT 1", DatabaseFamily::Sqlite);
        assert_eq!(sql, "EXPLAIN SELECT 1");
    }

    #[test]
    fn parses_postgres_json_tree() {
        let json = r#"[{
            "Plan": {
                "Node Type": "Limit",
                "Startup Cost": 0.00,
                "Total Cost": 3.80,
                "Plan Rows": 100,
                "Plans": [{
                    "Node Type": "Seq Scan",
                    "Relation Name": "ecrm_yb",
                    "Alias": "ecrm_yb",
                    "Startup Cost": 0.00,
                    "Total Cost": 19.00,
                    "Plan Rows": 500
                }]
            }
        }]"#;
        let result = QueryResult::rows(
            vec!["QUERY PLAN".into()],
            vec!["json".into()],
            vec![vec![QueryValue::String(json.into())]],
        );
        let plan = parse_explain_result(DatabaseFamily::Postgres, &result);
        assert_eq!(plan.node_count(), 2);
        assert_eq!(plan.roots[0].node_type, "Limit");
        assert_eq!(plan.roots[0].plan_rows, Some(100.0));
        assert_eq!(plan.roots[0].children[0].relation.as_deref(), Some("ecrm_yb"));
        assert_eq!(plan.estimated_total_cost(), Some(3.80));
        let expensive = plan.most_expensive().unwrap();
        assert_eq!(expensive.node_type, "Seq Scan");
        assert!(plan.raw.contains("Node Type"));
    }

    #[test]
    fn parses_sqlite_query_plan() {
        let result = QueryResult::rows(
            vec!["id".into(), "parent".into(), "notused".into(), "detail".into()],
            vec!["INT".into(), "INT".into(), "INT".into(), "TEXT".into()],
            vec![
                vec![
                    QueryValue::Int(2),
                    QueryValue::Int(0),
                    QueryValue::Int(0),
                    QueryValue::String("SCAN users".into()),
                ],
            ],
        );
        let plan = parse_explain_result(DatabaseFamily::Sqlite, &result);
        assert_eq!(plan.roots.len(), 1);
        assert_eq!(plan.roots[0].node_type, "Seq Scan");
        assert_eq!(plan.roots[0].relation.as_deref(), Some("users"));
    }
}
