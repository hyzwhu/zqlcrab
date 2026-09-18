//! Tabular data viewer component for displaying query results and table records.

use crate::db::types::{QueryResult, QueryValue};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::*, px,
};

#[derive(IntoElement)]
pub struct DataGrid {
    result: Option<QueryResult>,
    current_table: Option<String>,
    page_size: usize,
    current_page: usize,
}

impl DataGrid {
    pub fn new(result: Option<QueryResult>) -> Self {
        Self {
            result,
            current_table: None,
            page_size: 50,
            current_page: 0,
        }
    }

    pub fn table_name(mut self, name: Option<String>) -> Self {
        self.current_table = name;
        self
    }
}

impl RenderOnce for DataGrid {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        let Some(ref result) = self.result else {
            return v_flex()
                .size_full()
                .justify_center()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .p_4()
                        .rounded_full()
                        .bg(ThemeColors::BG_SURFACE)
                        .child(
                            Icon::new(IconName::Database)
                                .size(px(32.0))
                                .text_color(ThemeColors::TEXT_FAINT),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("No Query Results Yet"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("Execute a SQL query in the console or click a table to view rows."),
                );
        };

        if result.columns.is_empty() && result.rows.is_empty() {
            let affected = result.rows_affected.unwrap_or(0);
            return v_flex()
                .size_full()
                .justify_center()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::CircleCheck)
                        .size(px(28.0))
                        .text_color(ThemeColors::SUCCESS),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(format!("Query executed successfully: {affected} row(s) affected.")),
                )
                .when_some(result.execution_time_ms, |this, time| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("Duration: {time} ms")),
                    )
                });
        }

        let total_rows = result.rows.len();
        let _total_pages = if total_rows == 0 {
            1
        } else {
            (total_rows + self.page_size - 1) / self.page_size
        };
        let page_start = (self.current_page * self.page_size).min(total_rows);
        let page_end = (page_start + self.page_size).min(total_rows);
        let page_slice = &result.rows[page_start..page_end];

        // Header toolbar
        let title_label = if let Some(ref tbl) = self.current_table {
            format!("Table: {tbl} ({total_rows} rows)")
        } else {
            format!("Result ({total_rows} rows)")
        };

        let toolbar = h_flex()
            .w_full()
            .p_2()
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Table)
                            .size(px(16.0))
                            .text_color(ThemeColors::PRIMARY_BORDER),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(title_label),
                    )
                    .when_some(result.execution_time_ms, |this, time| {
                        this.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!("{time} ms")),
                        )
                    }),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("Showing {page_start}..{page_end} of {total_rows}")),
                    ),
            );

        // Header row
        let mut header_row = TableRow::new().child(
            TableHead::new()
                .child(
                    div()
                        .w(px(40.0))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child("#"),
                ),
        );

        for (i, col_name) in result.columns.iter().enumerate() {
            let col_type = result.column_types.get(i).map(|s| s.as_str()).unwrap_or("");
            header_row = header_row.child(
                TableHead::new().child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(col_name.clone()),
                        )
                        .when(!col_type.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child(format!("({col_type})")),
                            )
                        }),
                ),
            );
        }

        // Table body
        let mut body = TableBody::new();
        for (rel_idx, row_data) in page_slice.iter().enumerate() {
            let abs_idx = page_start + rel_idx + 1;
            let mut row = TableRow::new().child(
                TableCell::new().child(
                    div()
                        .w(px(40.0))
                        .text_xs()
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child(abs_idx.to_string()),
                ),
            );

            for val in row_data {
                let cell_elem = match val {
                    QueryValue::Null => div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child("NULL"),
                    QueryValue::Bool(b) => div()
                        .text_xs()
                        .text_color(if *b {
                            ThemeColors::SUCCESS
                        } else {
                            ThemeColors::WARNING
                        })
                        .child(if *b { "true" } else { "false" }),
                    QueryValue::Int(i) => div()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(i.to_string()),
                    QueryValue::Float(f) => div()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .text_color(ThemeColors::PRIMARY_BORDER)
                        .child(format!("{f:.4}")),
                    _ => div()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(val.to_display_string()),
                };
                row = row.child(TableCell::new().child(cell_elem));
            }
            body = body.child(row);
        }

        let table = Table::new()
            .small()
            .child(TableHeader::new().child(header_row))
            .child(body);

        v_flex()
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(toolbar)
            .child(
                div()
                    .id("data_grid_table_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .overflow_x_scroll()
                    .child(table),
            )
    }
}
