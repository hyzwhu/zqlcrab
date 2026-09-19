//! SQL pretty-printer used by Format SQL.

use sqlformat::{FormatOptions, Indent, QueryParams};

/// Formats SQL with 2-space indent and uppercase keywords.
pub fn format_sql(sql: &str) -> String {
    format_sql_with_indent(sql, 2)
}

/// Formats SQL with custom indentation spaces and uppercase keywords.
pub fn format_sql_with_indent(sql: &str, tab_size: usize) -> String {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let indent_spaces = tab_size.clamp(2, 8) as u8;

    let formatted = sqlformat::format(
        trimmed,
        &QueryParams::None,
        FormatOptions {
            indent: Indent::Spaces(indent_spaces),
            uppercase: true,
            lines_between_queries: 1,
        },
    );

    if formatted.trim().is_empty() {
        trimmed.to_string()
    } else {
        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_select_onto_multiple_lines() {
        let out = format_sql(r#"SELECT * FROM "public"."ecrm_yb" LIMIT 100;"#);
        assert!(out.to_uppercase().contains("SELECT"));
        assert!(out.contains("FROM"));
        assert!(out.contains("LIMIT"));
        assert!(out.lines().count() >= 3, "expected wrapped SQL, got: {out}");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(format_sql("   \n  "), "");
    }

    #[test]
    fn formats_with_custom_tab_size() {
        let out_4 = format_sql_with_indent("SELECT id, name FROM users", 4);
        assert!(out_4.contains("SELECT"));
        let out_8 = format_sql_with_indent("SELECT id, name FROM users", 8);
        assert!(out_8.contains("SELECT"));
    }
}
