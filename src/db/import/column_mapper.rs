//! Column mapping heuristics for associating source file fields with target table columns.

use crate::db::types::ColumnInfo;
use serde::{Deserialize, Serialize};

/// Mapping association between a source CSV column and a target database table column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnMapping {
    pub source_index: usize,
    pub source_header: String,
    pub target_column: Option<String>,
}

/// Normalizes identifier string by removing underscores, dashes, and converting to lowercase.
/// Allows matching `user_id`, `userId`, `UserId`, and `user-id`.
fn normalize_ident(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Automatically maps source CSV headers to target table columns using fuzzy and exact heuristics.
pub fn auto_map_columns(
    csv_headers: &[String],
    table_columns: &[ColumnInfo],
) -> Vec<ColumnMapping> {
    let mut used_targets = std::collections::HashSet::new();

    csv_headers
        .iter()
        .enumerate()
        .map(|(idx, header)| {
            let header_clean = header.trim();

            // 1. Exact match
            if let Some(col) = table_columns
                .iter()
                .find(|c| c.name == header_clean && !used_targets.contains(&c.name))
            {
                used_targets.insert(col.name.clone());
                return ColumnMapping {
                    source_index: idx,
                    source_header: header.clone(),
                    target_column: Some(col.name.clone()),
                };
            }

            // 2. Case-insensitive match
            if let Some(col) = table_columns.iter().find(|c| {
                c.name.eq_ignore_ascii_case(header_clean) && !used_targets.contains(&c.name)
            }) {
                used_targets.insert(col.name.clone());
                return ColumnMapping {
                    source_index: idx,
                    source_header: header.clone(),
                    target_column: Some(col.name.clone()),
                };
            }

            // 3. Normalized alphanumeric match (e.g. `order_id` matches `orderId`)
            let norm_header = normalize_ident(header_clean);
            if !norm_header.is_empty() {
                if let Some(col) = table_columns.iter().find(|c| {
                    normalize_ident(&c.name) == norm_header && !used_targets.contains(&c.name)
                }) {
                    used_targets.insert(col.name.clone());
                    return ColumnMapping {
                        source_index: idx,
                        source_header: header.clone(),
                        target_column: Some(col.name.clone()),
                    };
                }
            }

            // Unmatched
            ColumnMapping {
                source_index: idx,
                source_header: header.clone(),
                target_column: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_map_columns() {
        let headers = vec![
            "id".to_string(),
            "first_name".to_string(),
            "LastName".to_string(),
            "email_address".to_string(),
            "unrelated_field".to_string(),
        ];

        let cols = vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                is_nullable: false,
                is_primary_key: true,
                is_auto_increment: true,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "firstName".to_string(),
                data_type: "TEXT".to_string(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "last_name".to_string(),
                data_type: "TEXT".to_string(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
            ColumnInfo {
                name: "email_address".to_string(),
                data_type: "VARCHAR".to_string(),
                is_nullable: true,
                is_primary_key: false,
                is_auto_increment: false,
                default_value: None,
                description: None,
            },
        ];

        let mappings = auto_map_columns(&headers, &cols);
        assert_eq!(mappings[0].target_column.as_deref(), Some("id"));
        assert_eq!(mappings[1].target_column.as_deref(), Some("firstName"));
        assert_eq!(mappings[2].target_column.as_deref(), Some("last_name"));
        assert_eq!(mappings[3].target_column.as_deref(), Some("email_address"));
        assert_eq!(mappings[4].target_column, None);
    }
}
