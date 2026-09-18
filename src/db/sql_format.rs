//! SQL pretty-printer used by Format SQL.

use sqlformat::{FormatOptions, Indent, QueryParams};

/// Formats SQL with 2-space indent and uppercase keywords.
pub fn format_sql(sql: &str) -> String {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let formatted = sqlformat::format(
        trimmed,
        &QueryParams::None,
        FormatOptions {
            indent: Indent::Spaces(2),
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
}
