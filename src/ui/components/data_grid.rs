//! Tabular data viewer component for displaying query results and table records.

use crate::db::export::ExportFormat;
use crate::db::types::{QueryResult, QueryValue, SortDirection};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    scroll::ScrollableElement as _,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::*, px,
};
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(IntoElement)]
pub struct DataGrid {
    result: Option<QueryResult>,
    current_table: Option<String>,
    page_size: usize,
    current_page: usize,
    sort_column: Option<usize>,
    sort_direction: Option<SortDirection>,
    filter_keyword: String,
    on_sort: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_page_change: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_export: Option<Rc<dyn Fn(ExportFormat, &mut Window, &mut App) + 'static>>,
}

impl DataGrid {
    pub fn new(result: Option<QueryResult>) -> Self {
        Self {
            result,
            current_table: None,
            page_size: 50,
            current_page: 0,
            sort_column: None,
            sort_direction: None,
            filter_keyword: String::new(),
            on_sort: None,
            on_page_change: None,
            on_export: None,
        }
    }

    pub fn table_name(mut self, name: Option<String>) -> Self {
        self.current_table = name;
        self
    }

    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size = size.max(1);
        self
    }

    pub fn current_page(mut self, page: usize) -> Self {
        self.current_page = page;
        self
    }

    pub fn sort(mut self, col_idx: Option<usize>, dir: Option<SortDirection>) -> Self {
        self.sort_column = col_idx;
        self.sort_direction = dir;
        self
    }

    pub fn filter_keyword(mut self, keyword: impl Into<String>) -> Self {
        self.filter_keyword = keyword.into();
        self
    }

    pub fn on_sort<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_sort = Some(Rc::new(handler));
        self
    }

    pub fn on_page_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_page_change = Some(Rc::new(handler));
        self
    }

    pub fn on_export<F>(mut self, handler: F) -> Self
    where
        F: Fn(ExportFormat, &mut Window, &mut App) + 'static,
    {
        self.on_export = Some(Rc::new(handler));
        self
    }
}

/// Helper function to compare two QueryValues for sorting.
fn compare_query_values(a: &QueryValue, b: &QueryValue, dir: SortDirection) -> Ordering {
    let ord = match (a, b) {
        (QueryValue::Null, QueryValue::Null) => Ordering::Equal,
        (QueryValue::Null, _) => Ordering::Greater, // Nulls last in Ascending
        (_, QueryValue::Null) => Ordering::Less,
        (QueryValue::Bool(x), QueryValue::Bool(y)) => x.cmp(y),
        (QueryValue::Int(x), QueryValue::Int(y)) => x.cmp(y),
        (QueryValue::Float(x), QueryValue::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (QueryValue::Int(x), QueryValue::Float(y)) => (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal),
        (QueryValue::Float(x), QueryValue::Int(y)) => x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal),
        (QueryValue::String(x), QueryValue::String(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (QueryValue::DateTime(x), QueryValue::DateTime(y)) => x.cmp(y),
        (x, y) => x.to_display_string().to_lowercase().cmp(&y.to_display_string().to_lowercase()),
    };

    match dir {
        SortDirection::Ascending => ord,
        SortDirection::Descending => ord.reverse(),
    }
}

impl RenderOnce for DataGrid {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let result = match &self.result {
            Some(res) => res,
            None => {
                return v_flex()
                    .size_full()
                    .bg(ThemeColors::BG_APP)
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::Database)
                            .size(px(40.0))
                            .text_color(ThemeColors::TEXT_FAINT),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child("No query results yet"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child("Write a query in the editor and press ⌘↵ to inspect tabular data."),
                    );
            }
        };

        if result.columns.is_empty() {
            let affected = result.rows_affected.unwrap_or(0);
            return v_flex()
                .size_full()
                .bg(ThemeColors::BG_APP)
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::CircleCheck)
                        .size(px(36.0))
                        .text_color(ThemeColors::SUCCESS),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("Query executed successfully"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(format!("{affected} row(s) affected")),
                );
        }

        // Apply in-memory row filtering if a keyword is provided
        let kw = self.filter_keyword.trim().to_lowercase();
        let filtered_indices: Vec<usize> = if kw.is_empty() {
            (0..result.rows.len()).collect()
        } else {
            result
                .rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row.iter().any(|val| {
                        val.to_display_string().to_lowercase().contains(&kw)
                    })
                })
                .map(|(idx, _)| idx)
                .collect()
        };

        // Apply sorting
        let mut sorted_indices = filtered_indices;
        if let (Some(col_idx), Some(dir)) = (self.sort_column, self.sort_direction) {
            sorted_indices.sort_by(|&idx_a, &idx_b| {
                let val_a = result.rows[idx_a].get(col_idx).unwrap_or(&QueryValue::Null);
                let val_b = result.rows[idx_b].get(col_idx).unwrap_or(&QueryValue::Null);
                compare_query_values(val_a, val_b, dir)
            });
        }

        let total_matching = sorted_indices.len();
        let total_rows = result.rows.len();
        let total_pages = if total_matching == 0 {
            1
        } else {
            (total_matching + self.page_size - 1) / self.page_size
        };

        let current_page = self.current_page.min(total_pages.saturating_sub(1));
        let page_start = current_page * self.page_size;
        let page_end = (page_start + self.page_size).min(total_matching);
        let page_slice_indices = if total_matching == 0 {
            &[][..]
        } else {
            &sorted_indices[page_start..page_end]
        };

        // Export Buttons in Toolbar
        let export_csv_btn = {
            let on_exp = self.on_export.clone();
            Button::new("export_csv")
                .ghost()
                .xsmall()
                .icon(IconName::FileText)
                .tooltip("Export / Copy CSV")
                .child("CSV")
                .when_some(on_exp, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(ExportFormat::Csv, window, cx))
                })
        };

        let export_json_btn = {
            let on_exp = self.on_export.clone();
            Button::new("export_json")
                .ghost()
                .xsmall()
                .tooltip("Export / Copy JSON")
                .child("JSON")
                .when_some(on_exp, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(ExportFormat::Json, window, cx))
                })
        };

        let export_md_btn = {
            let on_exp = self.on_export.clone();
            Button::new("export_md")
                .ghost()
                .xsmall()
                .tooltip("Export / Copy Markdown")
                .child("MD")
                .when_some(on_exp, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(ExportFormat::Markdown, window, cx))
                })
        };

        let export_sql_btn = {
            let on_exp = self.on_export.clone();
            Button::new("export_sql")
                .ghost()
                .xsmall()
                .tooltip("Export / Copy SQL Inserts")
                .child("SQL")
                .when_some(on_exp, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(ExportFormat::SqlInsert, window, cx))
                })
        };

        // Pagination buttons
        let prev_page_btn = {
            let on_pg = self.on_page_change.clone();
            let is_disabled = current_page == 0;
            let mut btn = Button::new("prev_page")
                .ghost()
                .xsmall()
                .icon(IconName::ChevronLeft)
                .tooltip("Previous Page");
            if !is_disabled {
                if let Some(handler) = on_pg {
                    btn = btn.on_click(move |_, window, cx| {
                        if current_page > 0 {
                            handler(current_page - 1, window, cx);
                        }
                    });
                }
            }
            btn
        };

        let next_page_btn = {
            let on_pg = self.on_page_change.clone();
            let is_disabled = current_page + 1 >= total_pages;
            let mut btn = Button::new("next_page")
                .ghost()
                .xsmall()
                .icon(IconName::ChevronRight)
                .tooltip("Next Page");
            if !is_disabled {
                if let Some(handler) = on_pg {
                    btn = btn.on_click(move |_, window, cx| {
                        if current_page + 1 < total_pages {
                            handler(current_page + 1, window, cx);
                        }
                    });
                }
            }
            btn
        };

        // Grid Toolbar
        let toolbar = h_flex()
            .h(px(36.0))
            .w_full()
            .px_3()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .when_some(self.current_table.as_ref(), |this, table| {
                        this.child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(table.clone()),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(if kw.is_empty() {
                                format!("{total_rows} row(s)")
                            } else {
                                format!("{total_matching} of {total_rows} match")
                            }),
                    )
                    .when_some(result.execution_time_ms, |this, time| {
                        this.child(
                            div()
                                .px_1p5()
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
                    // Export Action Group
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(export_csv_btn)
                            .child(export_json_btn)
                            .child(export_md_btn)
                            .child(export_sql_btn),
                    )
                    .child(
                        div()
                            .w(px(1.0))
                            .h(px(16.0))
                            .bg(ThemeColors::BORDER),
                    )
                    // Pagination Navigation
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(prev_page_btn)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!("{}/{}", current_page + 1, total_pages)),
                            )
                            .child(next_page_btn),
                    ),
            );

        // Smart column width calculation to prevent text from overflowing or stacking
        let col_count = result.columns.len();
        let mut col_widths: Vec<f32> = Vec::with_capacity(col_count);

        for (i, col_name) in result.columns.iter().enumerate() {
            let col_type = result.column_types.get(i).map(|s| s.as_str()).unwrap_or("");
            let col_type_lower = col_type.to_lowercase();

            // Measure header text length (name + type + sort icon space + padding)
            let header_chars = col_name.chars().count()
                + if col_type.is_empty() {
                    0
                } else {
                    col_type.chars().count() + 3
                };
            let mut est_w = (header_chars as f32 * 8.5 + 44.0).max(80.0);

            // Sample visible rows (or up to 50 rows) to estimate content width
            for &orig_idx in page_slice_indices.iter().take(50) {
                if let Some(row) = result.rows.get(orig_idx) {
                    if let Some(val) = row.get(i) {
                        let s = val.to_display_string();
                        let char_count = s.chars().count();
                        let cjk_count = s.chars().filter(|c| !c.is_ascii()).count();
                        let ascii_count = char_count.saturating_sub(cjk_count);
                        let cell_w = (ascii_count as f32 * 8.0) + (cjk_count as f32 * 13.0) + 28.0;
                        if cell_w > est_w {
                            est_w = cell_w;
                        }
                    }
                }
            }

            // Determine min and max width based on column type
            let (min_w, max_w) = if col_type_lower.contains("bool") {
                (80.0, 120.0)
            } else if col_type_lower.contains("int") || col_type_lower.contains("serial") {
                (90.0, 160.0)
            } else if col_type_lower.contains("float")
                || col_type_lower.contains("double")
                || col_type_lower.contains("numeric")
                || col_type_lower.contains("decimal")
            {
                (100.0, 180.0)
            } else if col_type_lower.contains("date") || col_type_lower.contains("time") {
                (160.0, 240.0)
            } else if col_type_lower.contains("uuid") {
                (240.0, 320.0)
            } else {
                // text, varchar, json, etc.
                (140.0, 420.0)
            };

            col_widths.push(est_w.clamp(min_w, max_w));
        }

        // Fixed width for index column '#'
        let index_col_width: f32 = 48.0;

        // If total column width is less than a minimum comfortable viewport width (960px),
        // proportionally expand data columns so they fill the space nicely
        let min_total_viewport: f32 = 960.0;
        let data_cols_sum: f32 = col_widths.iter().sum();
        if data_cols_sum > 0.0 && data_cols_sum + index_col_width < min_total_viewport {
            let extra = (min_total_viewport - index_col_width) - data_cols_sum;
            let ratio = extra / data_cols_sum;
            for w in &mut col_widths {
                *w += *w * ratio;
            }
        }

        let total_table_width: f32 = col_widths.iter().sum::<f32>() + index_col_width;

        // Header row with sorting indicator and click handler
        let mut header_row = TableRow::new().child(
            TableHead::new()
                .w(px(index_col_width))
                .min_w(px(index_col_width))
                .flex_shrink_0()
                .overflow_hidden()
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child("#"),
                ),
        );

        for (i, col_name) in result.columns.iter().enumerate() {
            let col_w = col_widths.get(i).copied().unwrap_or(120.0);
            let col_type = result.column_types.get(i).map(|s| s.as_str()).unwrap_or("");
            let is_sorted = self.sort_column == Some(i);
            let sort_dir = if is_sorted { self.sort_direction } else { None };

            let on_sort_click = self.on_sort.clone();

            let sort_icon = match sort_dir {
                Some(SortDirection::Ascending) => Some(
                    Icon::new(IconName::ChevronUp)
                        .size(px(12.0))
                        .text_color(ThemeColors::PRIMARY_BORDER),
                ),
                Some(SortDirection::Descending) => Some(
                    Icon::new(IconName::ChevronDown)
                        .size(px(12.0))
                        .text_color(ThemeColors::PRIMARY_BORDER),
                ),
                None => None,
            };

            let header_content = h_flex()
                .w_full()
                .overflow_hidden()
                .items_center()
                .gap_1()
                .cursor_pointer()
                .child(
                    div()
                        .truncate()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if is_sorted {
                            ThemeColors::PRIMARY_BORDER
                        } else {
                            ThemeColors::TEXT_PRIMARY
                        })
                        .child(col_name.clone()),
                )
                .when(!col_type.is_empty(), |this| {
                    this.child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(format!("({col_type})")),
                    )
                })
                .children(sort_icon);

            let header_div = div()
                .w_full()
                .overflow_hidden()
                .id(gpui_kit::gpui::ElementId::NamedInteger("sort_col".into(), i as u64))
                .when_some(on_sort_click, |d, handler| {
                    d.on_click(move |_, window, cx| handler(i, window, cx))
                })
                .child(header_content);

            let head_cell = TableHead::new()
                .w(px(col_w))
                .min_w(px(col_w))
                .flex_shrink_0()
                .overflow_hidden()
                .child(header_div);

            header_row = header_row.child(head_cell);
        }

        // Table body
        let mut body = TableBody::new();
        for (rel_idx, &orig_row_idx) in page_slice_indices.iter().enumerate() {
            let abs_idx = page_start + rel_idx + 1;
            let row_data = &result.rows[orig_row_idx];

            let mut row = TableRow::new().child(
                TableCell::new()
                    .w(px(index_col_width))
                    .min_w(px(index_col_width))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(abs_idx.to_string()),
                    ),
            );

            for (col_idx, val) in row_data.iter().enumerate() {
                let col_w = col_widths.get(col_idx).copied().unwrap_or(120.0);
                let cell_elem = match val {
                    QueryValue::Null => div()
                        .w_full()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child("NULL"),
                    QueryValue::Bool(b) => div()
                        .w_full()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(if *b {
                            ThemeColors::SUCCESS
                        } else {
                            ThemeColors::WARNING
                        })
                        .child(if *b { "true" } else { "false" }),
                    QueryValue::Int(i) => div()
                        .w_full()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(i.to_string()),
                    QueryValue::Float(f) => div()
                        .w_full()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .text_color(ThemeColors::PRIMARY_BORDER)
                        .child(format!("{f:.4}")),
                    _ => div()
                        .w_full()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(val.to_display_string()),
                };

                row = row.child(
                    TableCell::new()
                        .w(px(col_w))
                        .min_w(px(col_w))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child(cell_elem),
                );
            }
            body = body.child(row);
        }

        let table = Table::new()
            .small()
            .min_w(px(total_table_width))
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
                    .overflow_scrollbar()
                    .child(table),
            )
    }
}
