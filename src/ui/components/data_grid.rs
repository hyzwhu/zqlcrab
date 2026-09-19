//! Tabular data viewer component for displaying query results and table records.

use crate::db::changeset::{GridChangeset, InsertAnchor, RowInsertion};
use crate::db::export::ExportFormat;
use crate::db::types::{QueryResult, QueryValue, SortDirection};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    scroll::{ScrollableElement as _, ScrollbarAxis},
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
};
use gpui_kit::gpui::{
    App, ClipboardItem, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, ScrollHandle, StatefulInteractiveElement as _, Styled, Window, div,
    point, prelude::*, px, rgba,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

/// Helper function to convert a single row into a formatted JSON object string.
pub fn row_to_json(columns: &[String], row: &[QueryValue]) -> String {
    let mut map = serde_json::Map::new();
    for (i, col_name) in columns.iter().enumerate() {
        let val = row.get(i).unwrap_or(&QueryValue::Null);
        let json_val = match val {
            QueryValue::Null => serde_json::Value::Null,
            QueryValue::Bool(b) => serde_json::Value::Bool(*b),
            QueryValue::Int(n) => serde_json::Value::Number((*n).into()),
            QueryValue::Float(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or_else(|| serde_json::Value::String(format!("{f}"))),
            QueryValue::String(s) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                    parsed
                } else {
                    serde_json::Value::String(s.clone())
                }
            }
            QueryValue::Bytes(b) => serde_json::Value::String(format!("<blob: {} bytes>", b.len())),
            QueryValue::DateTime(d) => serde_json::Value::String(d.clone()),
        };
        map.insert(col_name.clone(), json_val);
    }
    serde_json::to_string_pretty(&serde_json::Value::Object(map))
        .unwrap_or_else(|_| "{}".to_string())
}

/// Helper function to convert a single row into a TSV (tab-separated values) string.
pub fn row_to_tsv(row: &[QueryValue]) -> String {
    let parts: Vec<String> = row
        .iter()
        .map(|v| v.to_display_string().replace(['\t', '\r', '\n'], " "))
        .collect();
    parts.join("\t")
}

/// Formats a raw string value for inspection, detecting JSON and calculating statistics.
pub fn format_inspector_value(raw: &str, pretty_json: bool) -> (String, bool, usize, usize) {
    let char_count = raw.chars().count();
    let line_count = raw.lines().count().max(1);
    let trimmed = raw.trim();
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if pretty_json {
                if let Ok(pretty) = serde_json::to_string_pretty(&parsed) {
                    let p_chars = pretty.chars().count();
                    let p_lines = pretty.lines().count().max(1);
                    return (pretty, true, p_chars, p_lines);
                }
            }
            return (raw.to_string(), true, char_count, line_count);
        }
    }
    (raw.to_string(), false, char_count, line_count)
}

/// Represents an item in the displayed grid row sequence, either an existing table row or an inserted row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridDisplayItem {
    Original(usize, usize),
    Inserted(usize),
}

/// Computes the interleaved display items taking into account row insertion anchors.
pub fn compute_grid_display_items(
    page_slice_indices: &[usize],
    current_page: usize,
    total_pages: usize,
    inserted_rows: &[RowInsertion],
) -> Vec<GridDisplayItem> {
    let mut after_orig: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut after_ins: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut page_end: HashMap<usize, Vec<usize>> = HashMap::new();

    for (ins_idx, ins) in inserted_rows.iter().enumerate() {
        match ins.anchor {
            InsertAnchor::AfterRow(orig_idx) => {
                after_orig.entry(orig_idx).or_default().push(ins_idx);
            }
            InsertAnchor::AfterInserted(parent_temp_id) => {
                after_ins.entry(parent_temp_id).or_default().push(ins_idx);
            }
            InsertAnchor::PageEnd(page_idx) => {
                page_end.entry(page_idx).or_default().push(ins_idx);
            }
        }
    }

    fn append_inserted_subtree(
        ins_idx: usize,
        items: &mut Vec<GridDisplayItem>,
        after_ins: &HashMap<usize, Vec<usize>>,
        inserted_rows: &[RowInsertion],
        visited: &mut std::collections::HashSet<usize>,
    ) {
        if !visited.insert(ins_idx) {
            return;
        }
        items.push(GridDisplayItem::Inserted(ins_idx));
        if let Some(ins) = inserted_rows.get(ins_idx) {
            if let Some(children) = after_ins.get(&ins.temp_id) {
                for &child_idx in children {
                    append_inserted_subtree(child_idx, items, after_ins, inserted_rows, visited);
                }
            }
        }
    }

    let mut display_items = Vec::new();
    let mut visited_inserted = std::collections::HashSet::new();

    for (rel_idx, &orig_row_idx) in page_slice_indices.iter().enumerate() {
        display_items.push(GridDisplayItem::Original(rel_idx, orig_row_idx));
        if let Some(children) = after_orig.get(&orig_row_idx) {
            for &child_idx in children {
                append_inserted_subtree(
                    child_idx,
                    &mut display_items,
                    &after_ins,
                    inserted_rows,
                    &mut visited_inserted,
                );
            }
        }
    }

    if let Some(children) = page_end.get(&current_page) {
        for &child_idx in children {
            append_inserted_subtree(
                child_idx,
                &mut display_items,
                &after_ins,
                inserted_rows,
                &mut visited_inserted,
            );
        }
    }

    // Fallback for any unvisited inserted rows (e.g. empty table or page out of bounds)
    for ins_idx in 0..inserted_rows.len() {
        if !visited_inserted.contains(&ins_idx) {
            let on_other_page = match inserted_rows[ins_idx].anchor {
                InsertAnchor::PageEnd(p) => p != current_page && p < total_pages,
                _ => false,
            };
            if !on_other_page {
                append_inserted_subtree(
                    ins_idx,
                    &mut display_items,
                    &after_ins,
                    inserted_rows,
                    &mut visited_inserted,
                );
            }
        }
    }

    display_items
}

/// Unified coordinate identifying a cell in either an existing table row or an uncommitted new row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCellCoord {
    pub is_inserted: bool,
    pub row_idx: usize,
    pub col_idx: usize,
}

impl GridCellCoord {
    pub fn existing(row_idx: usize, col_idx: usize) -> Self {
        Self {
            is_inserted: false,
            row_idx,
            col_idx,
        }
    }

    pub fn inserted(insert_idx: usize, col_idx: usize) -> Self {
        Self {
            is_inserted: true,
            row_idx: insert_idx,
            col_idx,
        }
    }
}

#[derive(IntoElement)]
pub struct DataGrid {
    result: Option<QueryResult>,
    current_table: Option<String>,
    page_size: usize,
    current_page: usize,
    sort_column: Option<usize>,
    sort_direction: Option<SortDirection>,
    filter_keyword: String,
    selected_cell: Option<GridCellCoord>,
    inspector_open: bool,
    modal_open: bool,
    json_pretty: bool,
    changeset: GridChangeset,
    is_read_only: bool,
    cell_edit_input: Option<Entity<InputState>>,
    on_sort: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
    on_page_change: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
    on_export: Option<Rc<dyn Fn(ExportFormat, &mut Window, &mut App)>>,
    on_select_cell: Option<Rc<dyn Fn(GridCellCoord, &mut Window, &mut App)>>,
    on_toggle_inspector: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_toggle_modal: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_toggle_json_pretty: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_copy_value: Option<Rc<dyn Fn(String, String, &mut Window, &mut App)>>, // (col_name, value)
    on_copy_row_json: Option<Rc<dyn Fn(usize, String, &mut Window, &mut App)>>, // (row_idx, json)
    on_copy_row_tsv: Option<Rc<dyn Fn(usize, String, &mut Window, &mut App)>>, // (row_idx, tsv)
    on_apply_cell_edit: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_set_cell_null: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_revert_cell: Option<Rc<dyn Fn(usize, usize, &mut Window, &mut App)>>,
    on_toggle_delete_row: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
    on_add_row: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_duplicate_row: Option<Rc<dyn Fn(GridCellCoord, &mut Window, &mut App)>>,
    on_discard_inserted_row: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
    on_discard_all_changes: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_save_changes: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    scroll_handle: Option<ScrollHandle>,
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
            selected_cell: None,
            inspector_open: false,
            modal_open: false,
            json_pretty: true,
            changeset: GridChangeset::new(),
            is_read_only: false,
            cell_edit_input: None,
            on_sort: None,
            on_page_change: None,
            on_export: None,
            on_select_cell: None,
            on_toggle_inspector: None,
            on_toggle_modal: None,
            on_toggle_json_pretty: None,
            on_copy_value: None,
            on_copy_row_json: None,
            on_copy_row_tsv: None,
            on_apply_cell_edit: None,
            on_set_cell_null: None,
            on_revert_cell: None,
            on_toggle_delete_row: None,
            on_add_row: None,
            on_duplicate_row: None,
            on_discard_inserted_row: None,
            on_discard_all_changes: None,
            on_save_changes: None,
            scroll_handle: None,
        }
    }

    pub fn scroll_handle(mut self, handle: ScrollHandle) -> Self {
        self.scroll_handle = Some(handle);
        self
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

    pub fn sort(mut self, col: Option<usize>, dir: Option<SortDirection>) -> Self {
        self.sort_column = col;
        self.sort_direction = dir;
        self
    }

    pub fn filter_keyword(mut self, keyword: impl Into<String>) -> Self {
        self.filter_keyword = keyword.into();
        self
    }

    pub fn selected_cell(mut self, cell: Option<GridCellCoord>) -> Self {
        self.selected_cell = cell;
        self
    }

    pub fn inspector_open(mut self, open: bool) -> Self {
        self.inspector_open = open;
        self
    }

    pub fn modal_open(mut self, open: bool) -> Self {
        self.modal_open = open;
        self
    }

    pub fn json_pretty(mut self, pretty: bool) -> Self {
        self.json_pretty = pretty;
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

    pub fn on_select_cell<F>(mut self, handler: F) -> Self
    where
        F: Fn(GridCellCoord, &mut Window, &mut App) + 'static,
    {
        self.on_select_cell = Some(Rc::new(handler));
        self
    }

    pub fn on_add_row<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_add_row = Some(Rc::new(handler));
        self
    }

    pub fn on_duplicate_row<F>(mut self, handler: F) -> Self
    where
        F: Fn(GridCellCoord, &mut Window, &mut App) + 'static,
    {
        self.on_duplicate_row = Some(Rc::new(handler));
        self
    }

    pub fn on_discard_inserted_row<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_discard_inserted_row = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_inspector<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_inspector = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_modal<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_modal = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_json_pretty<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_json_pretty = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_value<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_value = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_row_json<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_row_json = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_row_tsv<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_row_tsv = Some(Rc::new(handler));
        self
    }

    pub fn changeset(mut self, changeset: GridChangeset) -> Self {
        self.changeset = changeset;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.is_read_only = read_only;
        self
    }

    pub fn cell_edit_input(mut self, input: Option<Entity<InputState>>) -> Self {
        self.cell_edit_input = input;
        self
    }

    pub fn on_apply_cell_edit<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_apply_cell_edit = Some(Rc::new(handler));
        self
    }

    pub fn on_set_cell_null<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_set_cell_null = Some(Rc::new(handler));
        self
    }

    pub fn on_revert_cell<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, usize, &mut Window, &mut App) + 'static,
    {
        self.on_revert_cell = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_delete_row<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_delete_row = Some(Rc::new(handler));
        self
    }

    pub fn on_discard_all_changes<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_discard_all_changes = Some(Rc::new(handler));
        self
    }

    pub fn on_save_changes<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_save_changes = Some(Rc::new(handler));
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
        (QueryValue::Int(x), QueryValue::Float(y)) => {
            (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        (QueryValue::Float(x), QueryValue::Int(y)) => {
            x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal)
        }
        (QueryValue::String(x), QueryValue::String(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (QueryValue::DateTime(x), QueryValue::DateTime(y)) => x.cmp(y),
        (x, y) => x
            .to_display_string()
            .to_lowercase()
            .cmp(&y.to_display_string().to_lowercase()),
    };

    match dir {
        SortDirection::Ascending => ord,
        SortDirection::Descending => ord.reverse(),
    }
}

impl RenderOnce for DataGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let result =
            match &self.result {
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
                        .child(div().text_xs().text_color(ThemeColors::TEXT_FAINT).child(
                            "Write a query in the editor and press ⌘↵ to inspect tabular data.",
                        ))
                        .into_any_element();
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
                )
                .into_any_element();
        }

        // Apply keyword filter
        let filter = self.filter_keyword.to_lowercase();
        let mut row_indices: Vec<usize> = if filter.is_empty() {
            (0..result.rows.len()).collect()
        } else {
            result
                .rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row.iter()
                        .any(|v| v.to_display_string().to_lowercase().contains(&filter))
                })
                .map(|(i, _)| i)
                .collect()
        };

        // Apply sorting
        if let (Some(col_idx), Some(dir)) = (self.sort_column, self.sort_direction) {
            if col_idx < result.columns.len() {
                row_indices.sort_by(|&a_idx, &b_idx| {
                    let a_val = &result.rows[a_idx][col_idx];
                    let b_val = &result.rows[b_idx][col_idx];
                    compare_query_values(a_val, b_val, dir)
                });
            }
        }

        let total_matching_rows = row_indices.len();
        let total_pages = (total_matching_rows + self.page_size - 1) / self.page_size;
        let total_pages = total_pages.max(1);
        let current_page = self.current_page.min(total_pages.saturating_sub(1));

        let page_start = current_page * self.page_size;
        let page_end = (page_start + self.page_size).min(total_matching_rows);
        let page_slice_indices = if page_start < total_matching_rows {
            &row_indices[page_start..page_end]
        } else {
            &[]
        };

        // Export handlers
        let on_exp = self.on_export.clone();
        let export_csv_btn = {
            let on_exp = on_exp.clone();
            Button::new("export_csv")
                .ghost()
                .xsmall()
                .flex_shrink(1.0)
                .min_w(px(26.0))
                .overflow_hidden()
                .tooltip("Export / Copy CSV")
                .icon(IconName::FileText)
                .child("CSV")
                .when_some(on_exp, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(ExportFormat::Csv, window, cx))
                })
        };

        let export_json_btn = {
            let on_exp = on_exp.clone();
            Button::new("export_json")
                .ghost()
                .xsmall()
                .flex_shrink(1.0)
                .min_w(px(26.0))
                .overflow_hidden()
                .tooltip("Export / Copy JSON")
                .child("JSON")
                .when_some(on_exp, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(ExportFormat::Json, window, cx))
                })
        };

        let export_md_btn = {
            let on_exp = on_exp.clone();
            Button::new("export_md")
                .ghost()
                .xsmall()
                .flex_shrink(1.0)
                .min_w(px(26.0))
                .overflow_hidden()
                .tooltip("Export / Copy Markdown")
                .child("MD")
                .when_some(on_exp, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(ExportFormat::Markdown, window, cx))
                })
        };

        let export_sql_btn = {
            let on_exp = on_exp.clone();
            Button::new("export_sql")
                .ghost()
                .xsmall()
                .flex_shrink(1.0)
                .min_w(px(26.0))
                .overflow_hidden()
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

        // Inspector toggle button
        let toggle_inspector_btn = {
            let on_tog = self.on_toggle_inspector.clone();
            let is_open = self.inspector_open;
            Button::new("toggle_inspector_toolbar_btn")
                .ghost()
                .xsmall()
                .flex_shrink(1.0)
                .min_w(px(26.0))
                .overflow_hidden()
                .icon(IconName::PanelRight)
                .label(if is_open { "Hide Details" } else { "Details" })
                .tooltip("Toggle Cell & Row Details Inspector")
                .border_1()
                .border_color(if is_open {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BORDER
                })
                .bg(if is_open {
                    ThemeColors::PRIMARY_BG
                } else {
                    ThemeColors::BG_SURFACE
                })
                .when_some(on_tog, move |btn, handler| {
                    btn.on_click(move |_, window, cx| {
                        handler(!is_open, window, cx);
                    })
                })
        };

        // Quick copy selected cell button in toolbar if a cell is selected
        let selected_info_pill = if let Some(coord) = self.selected_cell {
            if coord.is_inserted {
                if let Some(ins_row) = self.changeset.inserted_rows.get(coord.row_idx) {
                    let col_name = result
                        .columns
                        .get(coord.col_idx)
                        .cloned()
                        .unwrap_or_else(|| format!("col_{}", coord.col_idx));
                    let cur_val = ins_row
                        .values
                        .get(coord.col_idx)
                        .unwrap_or(&QueryValue::Null);
                    let raw_val = cur_val.to_display_string();
                    let on_cp = self.on_copy_value.clone();
                    let col_name_clone = col_name.clone();
                    let raw_val_clone = raw_val.clone();

                    Some(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_0p5()
                            .flex_shrink(1.0)
                            .min_w_0()
                            .overflow_hidden()
                            .rounded_md()
                            .bg(rgba(0x10B9811C))
                            .border_1()
                            .border_color(rgba(0x10B98150))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::SUCCESS)
                                    .flex_shrink_0()
                                    .child(format!("New Row #{}", coord.row_idx + 1)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .flex_shrink_0()
                                    .child("·"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .flex_shrink(1.0)
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(col_name),
                            )
                            .child(
                                Button::new("quick_copy_toolbar_btn")
                                    .ghost()
                                    .xsmall()
                                    .flex_shrink(1.0)
                                    .min_w(px(24.0))
                                    .overflow_hidden()
                                    .icon(IconName::Copy)
                                    .label("Copy")
                                    .tooltip("Copy selected cell value")
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref h) = on_cp {
                                            h(
                                                col_name_clone.clone(),
                                                raw_val_clone.clone(),
                                                window,
                                                cx,
                                            );
                                        } else {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                raw_val_clone.clone(),
                                            ));
                                        }
                                    }),
                            ),
                    )
                } else {
                    None
                }
            } else if coord.row_idx < result.rows.len() && coord.col_idx < result.columns.len() {
                let sel_row = coord.row_idx;
                let sel_col = coord.col_idx;
                let col_name = &result.columns[sel_col];
                let orig_val = &result.rows[sel_row][sel_col];
                let effective_val = self
                    .changeset
                    .get_effective_cell_value(sel_row, sel_col, orig_val);
                let raw_val = effective_val.to_display_string();
                let on_cp = self.on_copy_value.clone();
                let col_name_clone = col_name.clone();
                let raw_val_clone = raw_val.clone();
                let is_dirty = self.changeset.is_cell_dirty(sel_row, sel_col);

                Some(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .px_2()
                        .py_0p5()
                        .flex_shrink(1.0)
                        .min_w_0()
                        .overflow_hidden()
                        .rounded_md()
                        .bg(ThemeColors::BG_APP)
                        .when(is_dirty, |this| this.bg(rgba(0xF59E0B1A)))
                        .border_1()
                        .border_color(if is_dirty {
                            ThemeColors::WARNING
                        } else {
                            ThemeColors::BORDER
                        })
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_dirty {
                                    ThemeColors::WARNING
                                } else {
                                    ThemeColors::TEXT_MUTED
                                })
                                .flex_shrink(1.0)
                                .min_w_0()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(format!("#{}/{}", sel_row + 1, col_name)),
                        )
                        .child(
                            Button::new("quick_copy_toolbar_btn")
                                .ghost()
                                .xsmall()
                                .flex_shrink(1.0)
                                .min_w(px(24.0))
                                .overflow_hidden()
                                .icon(IconName::Copy)
                                .label("Copy")
                                .tooltip("Copy selected cell value")
                                .on_click(move |_, window, cx| {
                                    if let Some(ref h) = on_cp {
                                        h(
                                            col_name_clone.clone(),
                                            raw_val_clone.clone(),
                                            window,
                                            cx,
                                        );
                                    } else {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            raw_val_clone.clone(),
                                        ));
                                    }
                                }),
                        ),
                )
            } else {
                None
            }
        } else {
            None
        };

        // Add New Row button
        let add_new_row_btn = if !self.is_read_only {
            let on_add = self.on_add_row.clone();
            Some(
                Button::new("grid_add_new_row_btn")
                    .outline()
                    .xsmall()
                    .flex_shrink(1.0)
                    .min_w(px(28.0))
                    .overflow_hidden()
                    .icon(IconName::Plus)
                    .label("New Row")
                    .tooltip("Add a new uncommitted row (⌘N)")
                    .when_some(on_add, |btn, handler| {
                        btn.on_click(move |_, window, cx| {
                            handler(window, cx);
                        })
                    }),
            )
        } else {
            None
        };

        // Duplicate Selected Row button (uses selected row as template for new insertion)
        let duplicate_row_btn = if !self.is_read_only {
            if let Some(coord) = self.selected_cell {
                let on_dup = self.on_duplicate_row.clone();
                Some(
                    Button::new("grid_duplicate_row_btn")
                        .outline()
                        .xsmall()
                        .flex_shrink(1.0)
                        .min_w(px(28.0))
                        .overflow_hidden()
                        .icon(IconName::Copy)
                        .label("Duplicate")
                        .tooltip("Duplicate selected row as new row template (⌘D)")
                        .when_some(on_dup, move |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(coord, window, cx);
                            })
                        }),
                )
            } else {
                None
            }
        } else {
            None
        };

        // Row delete / restore / discard toggle button in toolbar
        let delete_restore_btn = if !self.is_read_only {
            if let Some(coord) = self.selected_cell {
                if coord.is_inserted {
                    let on_discard = self.on_discard_inserted_row.clone();
                    let ins_idx = coord.row_idx;
                    Some(
                        Button::new("tb_discard_row_btn")
                            .outline()
                            .xsmall()
                            .flex_shrink(1.0)
                            .min_w(px(28.0))
                            .overflow_hidden()
                            .icon(IconName::Trash)
                            .label("Discard Row")
                            .tooltip("Discard this uncommitted new row")
                            .when_some(on_discard, move |btn, handler| {
                                btn.on_click(move |_, window, cx| {
                                    handler(ins_idx, window, cx);
                                })
                            }),
                    )
                } else {
                    let sel_row = coord.row_idx;
                    let on_tog_del = self.on_toggle_delete_row.clone();
                    let is_del = self.changeset.is_row_deleted(sel_row);
                    Some(
                        Button::new("tb_del_restore_btn")
                            .ghost()
                            .xsmall()
                            .flex_shrink(1.0)
                            .min_w(px(28.0))
                            .overflow_hidden()
                            .icon(if is_del {
                                IconName::Undo
                            } else {
                                IconName::Trash
                            })
                            .label(if is_del { "Restore Row" } else { "Delete Row" })
                            .tooltip(if is_del {
                                "Restore row from deletion"
                            } else {
                                "Mark row for deletion (staged)"
                            })
                            .when_some(on_tog_del, move |btn, handler| {
                                btn.on_click(move |_, window, cx| {
                                    handler(sel_row, window, cx);
                                })
                            }),
                    )
                }
            } else {
                None
            }
        } else {
            None
        };

        // Pending Changes Bar (staged updates, deletes, and inserts)
        let (effective_updates, deleted_rows, inserted_rows) = self.changeset.change_summary();
        let dirty_bar = if self.changeset.is_dirty() {
            let on_discard = self.on_discard_all_changes.clone();
            let on_save = self.on_save_changes.clone();

            let discard_btn = Button::new("grid_discard_all_btn")
                .ghost()
                .xsmall()
                .flex_shrink(1.0)
                .min_w(px(28.0))
                .overflow_hidden()
                .icon(IconName::Undo)
                .label("Discard All")
                .when_some(on_discard, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(window, cx))
                });

            let save_btn = Button::new("grid_save_changes_btn")
                .primary()
                .xsmall()
                .flex_shrink(1.0)
                .min_w(px(28.0))
                .overflow_hidden()
                .icon(IconName::Check)
                .label("Review & Save (⌘S)")
                .when_some(on_save, |btn, handler| {
                    btn.on_click(move |_, window, cx| handler(window, cx))
                });

            let mut summary_parts = Vec::new();
            if inserted_rows > 0 {
                summary_parts.push(format!(
                    "{inserted_rows} new row{}",
                    if inserted_rows != 1 { "s" } else { "" }
                ));
            }
            if effective_updates > 0 {
                summary_parts.push(format!(
                    "{effective_updates} update{}",
                    if effective_updates != 1 { "s" } else { "" }
                ));
            }
            if deleted_rows > 0 {
                summary_parts.push(format!(
                    "{deleted_rows} deletion{}",
                    if deleted_rows != 1 { "s" } else { "" }
                ));
            }
            let summary_text = summary_parts.join(", ");

            Some(
                h_flex()
                    .w_full()
                    .h(px(34.0))
                    .px_3()
                    .justify_between()
                    .items_center()
                    .bg(rgba(0xF59E0B14))
                    .border_b_1()
                    .border_color(rgba(0xF59E0B33))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(ThemeColors::WARNING),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::WARNING)
                                    .child(format!("Unsaved Staged Changes: {summary_text}")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child("· Staged in memory, review before executing"),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(discard_btn)
                            .child(save_btn),
                    ),
            )
        } else {
            None
        };

        // Grid Toolbar
        let toolbar = h_flex()
            .h(px(36.0))
            .w_full()
            .px_3()
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .gap_2()
            .overflow_hidden()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .flex_shrink(1.0)
                    .min_w_0()
                    .overflow_hidden()
                    .when_some(self.current_table.as_ref(), |this, table| {
                        this.child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .flex_shrink(1.0)
                                .min_w_0()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(table.clone()),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .flex_shrink_0()
                            .child(if filter.is_empty() {
                                format!("{} row(s)", result.rows.len())
                            } else {
                                format!("{total_matching_rows} of {} row(s)", result.rows.len())
                            }),
                    )
                    .when_some(result.execution_time_ms, |this, ms| {
                        this.child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(ThemeColors::BG_APP)
                                .text_xs()
                                .text_color(ThemeColors::TEXT_FAINT)
                                .flex_shrink_0()
                                .child(format!("{ms} ms")),
                        )
                    })
                    .children(add_new_row_btn)
                    .children(duplicate_row_btn)
                    .when_some(selected_info_pill, |this, pill| {
                        this.child(
                            div()
                                .flex_shrink(1.0)
                                .min_w_0()
                                .overflow_hidden()
                                .child(pill),
                        )
                    })
                    .children(delete_restore_btn),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .flex_shrink(1.0)
                    .min_w_0()
                    .overflow_hidden()
                    // Export Actions
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .flex_shrink(1.0)
                            .min_w_0()
                            .overflow_hidden()
                            .child(export_csv_btn)
                            .child(export_json_btn)
                            .child(export_md_btn)
                            .child(export_sql_btn),
                    )
                    .child(
                        div()
                            .w(px(1.0))
                            .h(px(16.0))
                            .bg(ThemeColors::BORDER)
                            .flex_shrink_0(),
                    )
                    // Pagination Navigation
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .flex_shrink_0()
                            .child(prev_page_btn)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!("{}/{}", current_page + 1, total_pages)),
                            )
                            .child(next_page_btn),
                    )
                    .child(
                        div()
                            .w(px(1.0))
                            .h(px(16.0))
                            .bg(ThemeColors::BORDER)
                            .flex_shrink_0(),
                    )
                    // Inspector Toggle
                    .child(toggle_inspector_btn),
            );

        // Smart column width calculation
        let col_count = result.columns.len();
        let mut col_widths: Vec<f32> = Vec::with_capacity(col_count);

        for (i, col_name) in result.columns.iter().enumerate() {
            let col_type = result.column_types.get(i).map(|s| s.as_str()).unwrap_or("");
            let col_type_lower = col_type.to_lowercase();

            let header_chars = col_name.chars().count();
            let mut est_w = (header_chars as f32 * 8.5 + 28.0).max(60.0);

            for &orig_idx in page_slice_indices.iter().take(50) {
                if let Some(row) = result.rows.get(orig_idx) {
                    if let Some(orig_val) = row.get(i) {
                        let eff_val = self
                            .changeset
                            .get_effective_cell_value(orig_idx, i, orig_val);
                        let s = eff_val.to_display_string();
                        let char_count = s.chars().count();
                        let ascii_count = s.bytes().filter(|b| *b < 128).count();
                        let cjk_count = char_count.saturating_sub(ascii_count);
                        let cell_w = (ascii_count as f32 * 8.0) + (cjk_count as f32 * 13.0) + 28.0;
                        if cell_w > est_w {
                            est_w = cell_w;
                        }
                    }
                }
            }

            let (min_w, max_w) = if col_type_lower.contains("bool") {
                (60.0, 100.0)
            } else if col_type_lower.contains("int")
                || col_type_lower.contains("serial")
                || col_type_lower.contains("long")
            {
                (60.0, 140.0)
            } else if col_type_lower.contains("float")
                || col_type_lower.contains("double")
                || col_type_lower.contains("numeric")
                || col_type_lower.contains("decimal")
            {
                (80.0, 160.0)
            } else if col_type_lower.contains("date") || col_type_lower.contains("time") {
                (120.0, 220.0)
            } else if col_type_lower.contains("uuid") {
                (180.0, 300.0)
            } else {
                (80.0, 360.0)
            };

            col_widths.push(est_w.clamp(min_w, max_w));
        }

        let index_col_width: f32 = 48.0;
        let min_total_viewport: f32 = 720.0;
        let data_cols_sum: f32 = col_widths.iter().sum();
        if data_cols_sum > 0.0 && data_cols_sum + index_col_width < min_total_viewport {
            let extra = (min_total_viewport - index_col_width) - data_cols_sum;
            let ratio = extra / data_cols_sum;
            for w in &mut col_widths {
                *w += *w * ratio;
            }
        }

        let total_table_width: f32 = col_widths.iter().sum::<f32>() + index_col_width;

        // Table header row
        let mut header_row = TableRow::new()
            .w_full()
            .min_w(px(total_table_width))
            .bg(ThemeColors::BG_SURFACE)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(
                TableHead::new()
                    .h(px(32.0))
                    .w(px(index_col_width))
                    .min_w(px(index_col_width))
                    .px_0()
                    .py_0()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .border_r_1()
                    .border_color(ThemeColors::BORDER_PROMINENT)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        h_flex()
                            .size_full()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child("#"),
                            )
                            .child(
                                div()
                                    .w(px(1.0))
                                    .h(px(18.0))
                                    .bg(ThemeColors::BORDER_PROMINENT)
                                    .flex_none(),
                            ),
                    ),
            );

        for (i, col_name) in result.columns.iter().enumerate() {
            let col_w = col_widths.get(i).copied().unwrap_or(120.0);
            let is_last = i + 1 == col_count;
            let sort_dir = if self.sort_column == Some(i) {
                self.sort_direction
            } else {
                None
            };

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
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if sort_dir.is_some() {
                            ThemeColors::PRIMARY_BORDER
                        } else {
                            ThemeColors::TEXT_PRIMARY
                        })
                        .child(col_name.clone()),
                )
                .children(sort_icon);

            let header_div = h_flex()
                .size_full()
                .items_center()
                .justify_between()
                .gap_1()
                .px_2()
                .cursor_pointer()
                .overflow_hidden()
                .hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER))
                .id(ElementId::NamedInteger("sort_col".into(), i as u64))
                .when_some(on_sort_click, |d, handler| {
                    d.on_click(move |_, window, cx| handler(i, window, cx))
                })
                .child(header_content);

            let head_cell = TableHead::new()
                .h(px(32.0))
                .px_0()
                .py_0()
                .overflow_hidden()
                .border_r_1()
                .border_color(ThemeColors::BORDER_PROMINENT)
                .bg(ThemeColors::BG_SURFACE)
                .when(is_last, |this| this.min_w(px(col_w)).flex_1())
                .when(!is_last, |this| {
                    this.w(px(col_w)).min_w(px(col_w)).flex_shrink_0()
                })
                .child(
                    h_flex()
                        .size_full()
                        .items_center()
                        .justify_between()
                        .child(header_div.flex_1().min_w_0())
                        .child(
                            div()
                                .w(px(1.0))
                                .h(px(18.0))
                                .bg(ThemeColors::BORDER_PROMINENT)
                                .flex_none(),
                        ),
                );

            header_row = header_row.child(head_cell);
        }

        // Table body rows
        let mut body = TableBody::new();

        let display_items = compute_grid_display_items(
            &page_slice_indices,
            self.current_page,
            total_pages,
            &self.changeset.inserted_rows,
        );

        for item in display_items {
            match item {
                GridDisplayItem::Inserted(ins_idx) => {
                    let insertion = &self.changeset.inserted_rows[ins_idx];
                    let is_row_selected = self
                        .selected_cell
                        .map(|c| c.is_inserted && c.row_idx == ins_idx)
                        .unwrap_or(false);
                    let on_sel_row = self.on_select_cell.clone();

                    let index_cell_btn = div()
                        .size_full()
                        .h(px(32.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .id(ElementId::NamedInteger(
                            "grid_ins_row_idx".into(),
                            ins_idx as u64,
                        ))
                        .on_click(move |_, window, cx| {
                            if let Some(ref h) = on_sel_row {
                                h(GridCellCoord::inserted(ins_idx, 0), window, cx);
                            }
                        })
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_xs()
                                .bg(rgba(0x10B98122))
                                .border_1()
                                .border_color(rgba(0x10B98160))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::SUCCESS)
                                .child("+ NEW"),
                        );

                    let mut row = TableRow::new()
                        .w_full()
                        .min_w(px(total_table_width))
                        .border_b_1()
                        .border_color(rgba(0x10B98140))
                        .bg(if is_row_selected {
                            rgba(0x10B98125)
                        } else {
                            rgba(0x10B9810E)
                        })
                        .child(
                            TableCell::new()
                                .h(px(32.0))
                                .w(px(index_col_width))
                                .min_w(px(index_col_width))
                                .px_0()
                                .py_0()
                                .flex_shrink_0()
                                .overflow_hidden()
                                .border_r_1()
                                .border_color(rgba(0x10B98140))
                                .child(index_cell_btn),
                        );

                    for (col_idx, _col_name) in result.columns.iter().enumerate() {
                        let is_last = col_idx + 1 == col_count;
                        let col_w = col_widths.get(col_idx).copied().unwrap_or(120.0);
                        let is_cell_selected = self
                            .selected_cell
                            .map(|c| c.is_inserted && c.row_idx == ins_idx && c.col_idx == col_idx)
                            .unwrap_or(false);
                        let cur_val = insertion.values.get(col_idx).unwrap_or(&QueryValue::Null);

                        let val_element = match cur_val {
                            QueryValue::Null => div()
                                .w_full()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .italic()
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
                                .child(cur_val.to_display_string().replace(['\r', '\n'], " ")),
                        };

                        let on_sel_cell = self.on_select_cell.clone();
                        let on_tog_modal = self.on_toggle_modal.clone();
                        let cell_container = div()
                            .size_full()
                            .h(px(32.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .overflow_hidden()
                            .relative()
                            .id(ElementId::NamedInteger(
                                format!("grid_ins_cell_{ins_idx}").into(),
                                col_idx as u64,
                            ))
                            .when(is_cell_selected, |this| {
                                this.bg(ThemeColors::PRIMARY_BG)
                                    .border_2()
                                    .border_color(ThemeColors::PRIMARY_BORDER)
                                    .rounded_xs()
                            })
                            .when(!is_cell_selected, |this| {
                                this.hover(|s| s.bg(rgba(0x10B98118)))
                            })
                            .on_click(move |event, window, cx| {
                                if let Some(ref h) = on_sel_cell {
                                    h(GridCellCoord::inserted(ins_idx, col_idx), window, cx);
                                }
                                if event.click_count() >= 2 {
                                    if let Some(ref h_m) = on_tog_modal {
                                        h_m(true, window, cx);
                                    }
                                }
                            })
                            .child(val_element);

                        row = row.child(
                            TableCell::new()
                                .h(px(32.0))
                                .px_0()
                                .py_0()
                                .overflow_hidden()
                                .border_r_1()
                                .border_color(rgba(0x10B98130))
                                .when(is_last, |this| this.min_w(px(col_w)).flex_1())
                                .when(!is_last, |this| {
                                    this.w(px(col_w)).min_w(px(col_w)).flex_shrink_0()
                                })
                                .child(cell_container),
                        );
                    }

                    body = body.child(row);
                }
                GridDisplayItem::Original(rel_idx, orig_row_idx) => {
                    let abs_idx = page_start + rel_idx + 1;
                    let row_data = &result.rows[orig_row_idx];
                    let is_row_selected = self
                        .selected_cell
                        .map(|c| !c.is_inserted && c.row_idx == orig_row_idx)
                        .unwrap_or(false);
                    let is_row_deleted = self.changeset.is_row_deleted(orig_row_idx);

                    // Index column cell with row select handler
                    let on_sel_row = self.on_select_cell.clone();
                    let index_cell_btn = div()
                        .size_full()
                        .h(px(32.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .id(ElementId::NamedInteger(
                            "row_num_select".into(),
                            orig_row_idx as u64,
                        ))
                        .hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER))
                        .on_click(move |_, window, cx| {
                            if let Some(ref h) = on_sel_row {
                                h(GridCellCoord::existing(orig_row_idx, 0), window, cx);
                            }
                        })
                        .child(if is_row_deleted {
                            div()
                                .px_1()
                                .py_0p5()
                                .rounded_xs()
                                .bg(rgba(0xEF444425))
                                .border_1()
                                .border_color(rgba(0xEF444450))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::ERROR)
                                .child("DEL")
                        } else {
                            div()
                                .text_xs()
                                .font_weight(if is_row_selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(if is_row_selected {
                                    ThemeColors::PRIMARY_BORDER
                                } else {
                                    ThemeColors::TEXT_FAINT
                                })
                                .child(abs_idx.to_string())
                        });

                    let mut row = TableRow::new()
                        .w_full()
                        .min_w(px(total_table_width))
                        .border_b_1()
                        .border_color(ThemeColors::BORDER.opacity(0.35))
                        .when(is_row_deleted, |r| r.bg(rgba(0xEF444412)))
                        .when(!is_row_deleted && is_row_selected, |r| {
                            r.bg(ThemeColors::BG_SURFACE_ACTIVE)
                        })
                        .child(
                            TableCell::new()
                                .h(px(32.0))
                                .w(px(index_col_width))
                                .min_w(px(index_col_width))
                                .px_0()
                                .py_0()
                                .flex_shrink_0()
                                .overflow_hidden()
                                .border_r_1()
                                .border_color(ThemeColors::BORDER.opacity(0.35))
                                .child(index_cell_btn),
                        );

                    for (col_idx, orig_val) in row_data.iter().enumerate() {
                        let is_last = col_idx + 1 == col_count;
                        let col_w = col_widths.get(col_idx).copied().unwrap_or(120.0);
                        let is_cell_selected = self
                            .selected_cell
                            .map(|coord| {
                                !coord.is_inserted
                                    && coord.row_idx == orig_row_idx
                                    && coord.col_idx == col_idx
                            })
                            .unwrap_or(false);

                        let val = self.changeset.get_effective_cell_value(
                            orig_row_idx,
                            col_idx,
                            orig_val,
                        );
                        let is_cell_dirty = self.changeset.is_cell_dirty(orig_row_idx, col_idx);

                        let display_raw = val.to_display_string();
                        // Replace carriage return and newlines with space for clean single-line table display
                        let single_line = display_raw.replace(['\r', '\n'], " ");

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
                                .child(single_line),
                        };

                        let on_sel_cell = self.on_select_cell.clone();
                        let on_tog_modal = self.on_toggle_modal.clone();

                        let cell_container = div()
                            .size_full()
                            .h(px(32.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .overflow_hidden()
                            .relative()
                            .id(ElementId::NamedInteger(
                                "grid_cell_click".into(),
                                ((orig_row_idx as u64) << 24) | (col_idx as u64),
                            ))
                            .when(is_cell_selected, |this| {
                                this.bg(ThemeColors::PRIMARY_BG)
                                    .border_1()
                                    .border_color(ThemeColors::PRIMARY)
                                    .rounded_xs()
                            })
                            .when(!is_cell_selected && is_cell_dirty, |this| {
                                this.bg(rgba(0xF59E0B14))
                                    .border_1()
                                    .border_color(rgba(0xF59E0B50))
                                    .rounded_xs()
                            })
                            .when(!is_cell_selected && !is_cell_dirty, |this| {
                                this.hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER))
                            })
                            .on_click(move |event, window, cx| {
                                if let Some(ref h) = on_sel_cell {
                                    h(GridCellCoord::existing(orig_row_idx, col_idx), window, cx);
                                }
                                if event.click_count() >= 2 {
                                    if let Some(ref h_m) = on_tog_modal {
                                        h_m(true, window, cx);
                                    }
                                }
                            })
                            .child(if is_row_deleted {
                                div().w_full().opacity(0.4).child(cell_elem)
                            } else {
                                div().w_full().child(cell_elem)
                            })
                            .when(is_cell_dirty, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .top(px(2.0))
                                        .right(px(2.0))
                                        .w(px(5.0))
                                        .h(px(5.0))
                                        .rounded_full()
                                        .bg(rgba(0xF59E0BFF)),
                                )
                            });

                        row = row.child(
                            TableCell::new()
                                .h(px(32.0))
                                .px_0()
                                .py_0()
                                .overflow_hidden()
                                .border_r_1()
                                .border_color(ThemeColors::BORDER.opacity(0.35))
                                .when(is_last, |this| this.min_w(px(col_w)).flex_1())
                                .when(!is_last, |this| {
                                    this.w(px(col_w)).min_w(px(col_w)).flex_shrink_0()
                                })
                                .child(cell_container),
                        );
                    }
                    body = body.child(row);
                }
            }
        }

        let table = Table::new()
            .small()
            .w_full()
            .min_w(px(total_table_width))
            .child(
                TableHeader::new()
                    .w_full()
                    .min_w(px(total_table_width))
                    .bg(ThemeColors::BG_SURFACE)
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .child(header_row),
            )
            .child(body.w_full().min_w(px(total_table_width)));

        let scroll_handle = self.scroll_handle.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(
                    ElementId::Name("data_grid_table_scroll_handle".into()),
                    cx,
                    |_, _| ScrollHandle::default(),
                )
                .read(cx)
                .clone()
        });

        let table_wrap = div()
            .id("data_grid_table_inner_wrap")
            .w_full()
            .min_w(px(total_table_width))
            .min_h_full()
            .child(table);

        let scroll_area = div()
            .id("data_grid_table_scroll_area")
            .size_full()
            .overflow_x_scroll()
            .overflow_y_scroll()
            .track_scroll(&scroll_handle)
            .on_scroll_wheel({
                let scroll_handle = scroll_handle.clone();
                move |event, window, _| {
                    if event.modifiers.shift {
                        let p_delta = event.delta.pixel_delta(px(24.0));
                        if p_delta.y != px(0.0) && p_delta.x == px(0.0) {
                            let curr = scroll_handle.offset();
                            let max_off = scroll_handle.max_offset();
                            let new_x = (curr.x + p_delta.y).clamp(-max_off.x, px(0.0));
                            scroll_handle.set_offset(point(new_x, curr.y));
                            window.refresh();
                        }
                    }
                }
            })
            .child(table_wrap);

        let table_scroll_view = div()
            .id("data_grid_table_scroll")
            .flex_1()
            .h_full()
            .min_w_0()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .child(scroll_area)
            .scrollbar(&scroll_handle, ScrollbarAxis::Both);

        // Build Right Inspector Panel
        let inspector_panel = if self.inspector_open {
            if let Some(coord) = self.selected_cell {
                let sel_row = coord.row_idx;
                let sel_col = coord.col_idx;
                let is_inserted = coord.is_inserted;

                let row_vals_opt: Option<Vec<QueryValue>> = if is_inserted {
                    self.changeset
                        .inserted_rows
                        .get(sel_row)
                        .map(|r| r.values.clone())
                } else {
                    result.rows.get(sel_row).cloned()
                };

                if let Some(row_vals) = row_vals_opt.filter(|_| sel_col < result.columns.len()) {
                    let col_name = &result.columns[sel_col];
                    let col_type = result
                        .column_types
                        .get(sel_col)
                        .map(|s| s.as_str())
                        .unwrap_or("TEXT");
                    let current_val = row_vals.get(sel_col).unwrap_or(&QueryValue::Null);
                    let raw_val_str = current_val.to_display_string();
                    let (display_val_str, is_json, char_count, line_count) =
                        format_inspector_value(&raw_val_str, self.json_pretty);

                    // Copy Value button
                    let on_cp_val = self.on_copy_value.clone();
                    let col_name_cp = col_name.clone();
                    let val_cp = raw_val_str.clone();
                    let copy_val_btn = Button::new("insp_copy_val_btn")
                        .small()
                        .icon(IconName::Copy)
                        .label("Copy Value")
                        .tooltip("Copy unclipped raw cell value to clipboard")
                        .on_click(move |_, window, cx| {
                            if let Some(ref h) = on_cp_val {
                                h(col_name_cp.clone(), val_cp.clone(), window, cx);
                            } else {
                                cx.write_to_clipboard(ClipboardItem::new_string(val_cp.clone()));
                            }
                        });

                    // Maximize / Modal button
                    let on_tog_modal = self.on_toggle_modal.clone();
                    let max_btn = Button::new("insp_max_modal_btn")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Maximize2)
                        .tooltip("Expand to large modal view")
                        .on_click(move |_, window, cx| {
                            if let Some(ref h) = on_tog_modal {
                                h(true, window, cx);
                            }
                        });

                    // Close inspector button
                    let on_tog_insp = self.on_toggle_inspector.clone();
                    let close_insp_btn = Button::new("insp_close_x_btn")
                        .ghost()
                        .xsmall()
                        .icon(IconName::X)
                        .tooltip("Close Inspector")
                        .on_click(move |_, window, cx| {
                            if let Some(ref h) = on_tog_insp {
                                h(false, window, cx);
                            }
                        });

                    // JSON format toggle button
                    let json_toggle = if is_json {
                        let on_tog_pretty = self.on_toggle_json_pretty.clone();
                        let is_pretty = self.json_pretty;
                        Some(
                            Button::new("insp_json_format_toggle")
                                .ghost()
                                .xsmall()
                                .label(if is_pretty { "JSON Pretty" } else { "JSON Raw" })
                                .border_1()
                                .border_color(ThemeColors::BORDER)
                                .tooltip("Toggle between formatted JSON and raw string")
                                .on_click(move |_, window, cx| {
                                    if let Some(ref h) = on_tog_pretty {
                                        h(!is_pretty, window, cx);
                                    }
                                }),
                        )
                    } else {
                        None
                    };

                    // Copy row as JSON button
                    let on_cp_row_json = self.on_copy_row_json.clone();
                    let row_json_str = row_to_json(&result.columns, &row_vals);
                    let copy_row_json_btn = Button::new("insp_copy_row_json_btn")
                        .ghost()
                        .xsmall()
                        .icon(IconName::FileText)
                        .label("Row JSON")
                        .tooltip("Copy entire row as JSON object")
                        .on_click(move |_, window, cx| {
                            if !is_inserted {
                                if let Some(ref h) = on_cp_row_json {
                                    h(sel_row, row_json_str.clone(), window, cx);
                                    return;
                                }
                            }
                            cx.write_to_clipboard(ClipboardItem::new_string(row_json_str.clone()));
                        });

                    // Copy row as TSV button
                    let on_cp_row_tsv = self.on_copy_row_tsv.clone();
                    let row_tsv_str = row_to_tsv(&row_vals);
                    let copy_row_tsv_btn = Button::new("insp_copy_row_tsv_btn")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Table)
                        .label("Row TSV")
                        .tooltip("Copy entire row as TSV for spreadsheets")
                        .on_click(move |_, window, cx| {
                            if !is_inserted {
                                if let Some(ref h) = on_cp_row_tsv {
                                    h(sel_row, row_tsv_str.clone(), window, cx);
                                    return;
                                }
                            }
                            cx.write_to_clipboard(ClipboardItem::new_string(row_tsv_str.clone()));
                        });

                    // Inspector Duplicate row button
                    let insp_duplicate_row_btn = if !self.is_read_only {
                        let on_dup = self.on_duplicate_row.clone();
                        Some(
                            Button::new("insp_duplicate_row_btn")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Copy)
                                .label("Duplicate")
                                .tooltip("Duplicate this row as a new row template (⌘D)")
                                .when_some(on_dup, move |btn, handler| {
                                    btn.on_click(move |_, window, cx| {
                                        handler(coord, window, cx);
                                    })
                                }),
                        )
                    } else {
                        None
                    };

                    // Row columns overview list
                    let mut record_fields_list = v_flex().gap_1();
                    for (c_idx, c_name) in result.columns.iter().enumerate() {
                        let is_active_col = c_idx == sel_col;
                        let field_val = row_vals.get(c_idx).unwrap_or(&QueryValue::Null);
                        let field_str = field_val.to_display_string().replace(['\r', '\n'], " ");
                        let field_raw_val = field_val.to_display_string();

                        let on_sel = self.on_select_cell.clone();
                        let on_cp = self.on_copy_value.clone();
                        let col_name_c = c_name.clone();

                        let field_row = h_flex()
                            .w_full()
                            .p_1p5()
                            .rounded_sm()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .cursor_pointer()
                            .id(ElementId::NamedInteger(
                                "insp_col_select".into(),
                                c_idx as u64,
                            ))
                            .when(is_active_col, |this| {
                                this.bg(ThemeColors::PRIMARY_BG)
                                    .border_1()
                                    .border_color(ThemeColors::PRIMARY)
                            })
                            .when(!is_active_col, |this| {
                                this.bg(ThemeColors::BG_APP)
                                    .hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER))
                            })
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_sel {
                                    let coord = if is_inserted {
                                        GridCellCoord::inserted(sel_row, c_idx)
                                    } else {
                                        GridCellCoord::existing(sel_row, c_idx)
                                    };
                                    h(coord, window, cx);
                                }
                            })
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(if is_active_col {
                                                ThemeColors::PRIMARY_BORDER
                                            } else {
                                                ThemeColors::TEXT_MUTED
                                            })
                                            .child(c_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .font_family("JetBrains Mono")
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(field_str),
                                    ),
                            )
                            .child(
                                Button::new(ElementId::NamedInteger(
                                    "insp_copy_col_small".into(),
                                    c_idx as u64,
                                ))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Copy)
                                .tooltip("Copy this field")
                                .on_click(move |_, window, cx| {
                                    if let Some(ref h) = on_cp {
                                        h(col_name_c.clone(), field_raw_val.clone(), window, cx);
                                    } else {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            field_raw_val.clone(),
                                        ));
                                    }
                                }),
                            );

                        record_fields_list = record_fields_list.child(field_row);
                    }

                    Some(
                        v_flex()
                            .w(px(340.0))
                            .min_w(px(340.0))
                            .h_full()
                            .flex_shrink_0()
                            .overflow_hidden()
                            .border_l_1()
                            .border_color(ThemeColors::BORDER)
                            .bg(ThemeColors::BG_SURFACE)
                            // Inspector Header
                            .child(
                                h_flex()
                                    .h(px(36.0))
                                    .w_full()
                                    .px_3()
                                    .items_center()
                                    .justify_between()
                                    .border_b_1()
                                    .border_color(ThemeColors::BORDER)
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(Icon::new(IconName::Table).size(px(14.0)).text_color(ThemeColors::PRIMARY_BORDER))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("Value Inspector"),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(if is_inserted { rgba(0x10B98122).into() } else { ThemeColors::BG_APP })
                                                    .text_xs()
                                                    .font_weight(if is_inserted { FontWeight::BOLD } else { FontWeight::NORMAL })
                                                    .text_color(if is_inserted { ThemeColors::SUCCESS } else { ThemeColors::TEXT_FAINT })
                                                    .child(if is_inserted {
                                                        format!("New Row #{}", sel_row + 1)
                                                    } else {
                                                        format!("Row #{}", sel_row + 1)
                                                    }),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1()
                                            .child(max_btn)
                                            .child(close_insp_btn),
                                    ),
                            )
                            // Inspector Content Area
                            .child(
                                v_flex()
                                    .flex_1()
                                    .w_full()
                                    .min_w_0()
                                    .min_h_0()
                                    .overflow_scrollbar()
                                    .p_3()
                                    .gap_3()
                                    // Selected Field Overview Card
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .min_w_0()
                                            .gap_2()
                                            .p_2p5()
                                            .rounded_md()
                                            .bg(ThemeColors::BG_APP)
                                            .border_1()
                                            .border_color(ThemeColors::BORDER)
                                            .child(
                                                h_flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                                    .child(col_name.clone()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px_1p5()
                                                                    .py_0p5()
                                                                    .rounded_sm()
                                                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                                                    .text_xs()
                                                                    .text_color(ThemeColors::TEXT_MUTED)
                                                                    .child(col_type.to_string()),
                                                            ),
                                                    )
                                                    .children(json_toggle),
                                            )
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .text_xs()
                                                    .text_color(ThemeColors::TEXT_FAINT)
                                                    .child(format!("{char_count} chars"))
                                                    .child("·")
                                                    .child(format!("{line_count} line(s)")),
                                            ),
                                    )
                                    // Full Value Display Box
                                    .child(
                                        div()
                                            .w_full()
                                            .min_w_0()
                                            .min_h(px(120.0))
                                            .max_h(px(200.0))
                                            .p_2p5()
                                            .rounded_md()
                                            .bg(ThemeColors::BG_APP)
                                            .border_1()
                                            .border_color(ThemeColors::BORDER)
                                            .overflow_scrollbar()
                                            .child(
                                                if current_val.is_null() {
                                                    div()
                                                        .text_xs()
                                                        .text_color(ThemeColors::TEXT_FAINT)
                                                        .child("<NULL>")
                                                } else {
                                                    div()
                                                        .text_xs()
                                                        .font_family("JetBrains Mono")
                                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                                        .child(display_val_str)
                                                },
                                            ),
                                    )
                                    // Action Bar for Current Value
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .justify_end()
                                            .child(copy_val_btn),
                                    )
                                    // Live Cell Value Editor Section
                                    .child(
                                        if self.is_read_only {
                                            div()
                                                .p_2()
                                                .rounded_md()
                                                .bg(ThemeColors::BG_APP)
                                                .border_1()
                                                .border_color(ThemeColors::BORDER)
                                                .child(
                                                    h_flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(Icon::new(IconName::Lock).size(px(12.0)).text_color(ThemeColors::TEXT_FAINT))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(ThemeColors::TEXT_FAINT)
                                                                .child("Read-only connection: cell editing disabled"),
                                                        ),
                                                )
                                        } else {
                                            let is_cell_dirty = if is_inserted {
                                                true
                                            } else {
                                                self.changeset.is_cell_dirty(sel_row, sel_col)
                                            };
                                            let is_row_del = if is_inserted {
                                                false
                                            } else {
                                                self.changeset.is_row_deleted(sel_row)
                                            };
                                            let on_app = self.on_apply_cell_edit.clone();
                                            let on_nul = self.on_set_cell_null.clone();
                                            let on_rev = self.on_revert_cell.clone();

                                            let mut apply_btn = Button::new("insp_apply_edit_btn")
                                                .primary()
                                                .xsmall()
                                                .icon(IconName::Check)
                                                .label("Apply (↵)");
                                            if let Some(ref h) = on_app {
                                                let h = h.clone();
                                                apply_btn = apply_btn.on_click(move |_, window, cx| {
                                                    h(window, cx);
                                                });
                                            }

                                            let mut null_btn = Button::new("insp_set_null_btn")
                                                .outline()
                                                .xsmall()
                                                .label("Set NULL");
                                            if let Some(ref h) = on_nul {
                                                let h = h.clone();
                                                null_btn = null_btn.on_click(move |_, window, cx| {
                                                    h(window, cx);
                                                });
                                            }

                                            let mut rev_btn = Button::new("insp_revert_cell_btn")
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::Undo)
                                                .label("Revert");
                                            if !is_inserted && is_cell_dirty {
                                                if let Some(ref h) = on_rev {
                                                    let h = h.clone();
                                                    rev_btn = rev_btn.on_click(move |_, window, cx| {
                                                        h(sel_row, sel_col, window, cx);
                                                    });
                                                }
                                            }

                                            div()
                                                .w_full()
                                                .min_w_0()
                                                .p_2p5()
                                                .rounded_md()
                                                .bg(ThemeColors::BG_APP)
                                                .border_1()
                                                .border_color(if is_inserted {
                                                    rgba(0x10B98180).into()
                                                } else if is_cell_dirty {
                                                    ThemeColors::WARNING
                                                } else {
                                                    ThemeColors::BORDER
                                                })
                                                .child(
                                                    v_flex()
                                                        .gap_2()
                                                        .child(
                                                            h_flex()
                                                                .justify_between()
                                                                .items_center()
                                                                .child(
                                                                    h_flex()
                                                                        .items_center()
                                                                        .gap_2()
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                                .text_color(ThemeColors::TEXT_PRIMARY)
                                                                                .child("Edit Cell Value"),
                                                                        )
                                                                        .when(is_inserted, |this| {
                                                                            this.child(
                                                                                div()
                                                                                    .px_1p5()
                                                                                    .py_0p5()
                                                                                    .rounded_xs()
                                                                                    .bg(rgba(0x10B98120))
                                                                                    .border_1()
                                                                                    .border_color(rgba(0x10B98140))
                                                                                    .text_xs()
                                                                                    .font_weight(FontWeight::MEDIUM)
                                                                                    .text_color(ThemeColors::SUCCESS)
                                                                                    .child("New Row"),
                                                                            )
                                                                        })
                                                                        .when(!is_inserted && is_cell_dirty, |this| {
                                                                            this.child(
                                                                                div()
                                                                                    .px_1p5()
                                                                                    .py_0p5()
                                                                                    .rounded_xs()
                                                                                    .bg(rgba(0xF59E0B20))
                                                                                    .border_1()
                                                                                    .border_color(rgba(0xF59E0B40))
                                                                                    .text_xs()
                                                                                    .font_weight(FontWeight::MEDIUM)
                                                                                    .text_color(ThemeColors::WARNING)
                                                                                    .child("Modified"),
                                                                            )
                                                                        })
                                                                        .when(!is_inserted && is_row_del, |this| {
                                                                            this.child(
                                                                                div()
                                                                                    .px_1p5()
                                                                                    .py_0p5()
                                                                                    .rounded_xs()
                                                                                    .bg(rgba(0xEF444420))
                                                                                    .border_1()
                                                                                    .border_color(rgba(0xEF444440))
                                                                                    .text_xs()
                                                                                    .font_weight(FontWeight::MEDIUM)
                                                                                    .text_color(ThemeColors::ERROR)
                                                                                    .child("Pending Deletion"),
                                                                            )
                                                                        }),
                                                                )
                                                                .child(
                                                                    h_flex()
                                                                        .items_center()
                                                                        .gap_1()
                                                                        .child(null_btn)
                                                                        .when(!is_inserted && is_cell_dirty, |this| this.child(rev_btn)),
                                                                ),
                                                        )
                                                        .when_some(self.cell_edit_input.clone(), |this, inp| {
                                                            this.child(Input::new(&inp).small().w_full())
                                                        })
                                                        .child(
                                                            h_flex()
                                                                .justify_end()
                                                                .items_center()
                                                                .child(apply_btn),
                                                        ),
                                                )
                                        },
                                    )
                                    // Row Record Fields Section
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .min_w_0()
                                            .gap_2()
                                            .pt_2()
                                            .border_t_1()
                                            .border_color(ThemeColors::BORDER)
                                            .child(
                                                h_flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(ThemeColors::TEXT_MUTED)
                                                            .child(if is_inserted {
                                                                format!("New Row #{} Columns", sel_row + 1)
                                                            } else {
                                                                format!("Row #{} Columns", sel_row + 1)
                                                            }),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_1()
                                                            .child(copy_row_json_btn)
                                                            .child(copy_row_tsv_btn)
                                                            .children(insp_duplicate_row_btn),
                                                    ),
                                            )
                                            .child(record_fields_list),
                                    ),
                            ),
                    )
                } else {
                    None
                }
            } else {
                // Empty selection placeholder in inspector
                Some(
                    v_flex()
                        .w(px(340.0))
                        .min_w(px(340.0))
                        .h_full()
                        .flex_shrink_0()
                        .overflow_hidden()
                        .border_l_1()
                        .border_color(ThemeColors::BORDER)
                        .bg(ThemeColors::BG_SURFACE)
                        .child(
                            h_flex()
                                .h(px(36.0))
                                .w_full()
                                .px_3()
                                .items_center()
                                .justify_between()
                                .border_b_1()
                                .border_color(ThemeColors::BORDER)
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                        .child("Value Inspector"),
                                )
                                .child({
                                    let on_tog_insp = self.on_toggle_inspector.clone();
                                    Button::new("insp_close_empty_btn")
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::X)
                                        .on_click(move |_, window, cx| {
                                            if let Some(ref h) = on_tog_insp {
                                                h(false, window, cx);
                                            }
                                        })
                                }),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .items_center()
                                .justify_center()
                                .p_6()
                                .gap_3()
                                .child(Icon::new(IconName::Pointer).size(px(32.0)).text_color(ThemeColors::TEXT_FAINT))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_MUTED)
                                        .text_center()
                                        .child("Click any row or cell in the table to inspect its complete content and copy values."),
                                ),
                        ),
                )
            }
        } else {
            None
        };

        // Main table and inspector layout
        let main_view = h_flex()
            .flex_1()
            .h_full()
            .min_h_0()
            .min_w_0()
            .w_full()
            .items_stretch()
            .overflow_hidden()
            .child(table_scroll_view)
            .children(inspector_panel);

        let grid_content = v_flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .bg(ThemeColors::BG_APP)
            .child(toolbar)
            .children(dirty_bar)
            .child(main_view);

        // Modal Detail Viewer overlay if modal_open is true
        if self.modal_open {
            if let Some(coord) = self.selected_cell {
                let sel_row = coord.row_idx;
                let sel_col = coord.col_idx;
                let is_inserted = coord.is_inserted;

                let row_vals_opt: Option<Vec<QueryValue>> = if is_inserted {
                    self.changeset
                        .inserted_rows
                        .get(sel_row)
                        .map(|r| r.values.clone())
                } else {
                    result.rows.get(sel_row).cloned()
                };

                if let Some(row_vals) = row_vals_opt.filter(|_| sel_col < result.columns.len()) {
                    let col_name = &result.columns[sel_col];
                    let col_type = result
                        .column_types
                        .get(sel_col)
                        .map(|s| s.as_str())
                        .unwrap_or("TEXT");
                    let current_val = row_vals.get(sel_col).unwrap_or(&QueryValue::Null);
                    let raw_val_str = current_val.to_display_string();
                    let (display_val_str, is_json, char_count, line_count) =
                        format_inspector_value(&raw_val_str, self.json_pretty);

                    let on_cp_val = self.on_copy_value.clone();
                    let col_name_cp = col_name.clone();
                    let val_cp = raw_val_str.clone();

                    let on_tog_modal = self.on_toggle_modal.clone();
                    let on_tog_pretty = self.on_toggle_json_pretty.clone();
                    let is_pretty = self.json_pretty;

                    let modal_dialog = v_flex()
                        .w(px(800.0))
                        .h(px(560.0))
                        .bg(ThemeColors::BG_SURFACE)
                        .rounded_xl()
                        .border_1()
                        .border_color(ThemeColors::BORDER)
                        .shadow_lg()
                        .child(
                            // Modal Header
                            h_flex()
                                .w_full()
                                .p_4()
                                .justify_between()
                                .items_center()
                                .border_b_1()
                                .border_color(ThemeColors::BORDER)
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_3()
                                        .child(
                                            Icon::new(IconName::Table)
                                                .size(px(20.0))
                                                .text_color(ThemeColors::PRIMARY_BORDER),
                                        )
                                        .child(
                                            v_flex()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                                        .child(format!(
                                                            "Detail Viewer: {col_name}"
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(ThemeColors::TEXT_MUTED)
                                                        .child(format!(
                                                            "{col_type} · {}",
                                                            if is_inserted {
                                                                format!("New Row #{}", sel_row + 1)
                                                            } else {
                                                                format!("Row #{}", sel_row + 1)
                                                            }
                                                        )),
                                                ),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_2()
                                        .when(is_json, |this| {
                                            let on_tog_pretty = on_tog_pretty.clone();
                                            this.child(
                                                Button::new("modal_json_toggle_btn")
                                                    .ghost()
                                                    .xsmall()
                                                    .label(if is_pretty {
                                                        "JSON Pretty"
                                                    } else {
                                                        "JSON Raw"
                                                    })
                                                    .border_1()
                                                    .border_color(ThemeColors::BORDER)
                                                    .on_click(move |_, window, cx| {
                                                        if let Some(ref h) = on_tog_pretty {
                                                            h(!is_pretty, window, cx);
                                                        }
                                                    }),
                                            )
                                        })
                                        .child({
                                            let on_tog_modal = on_tog_modal.clone();
                                            Button::new("modal_close_x_btn")
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::X)
                                                .on_click(move |_, window, cx| {
                                                    if let Some(ref h) = on_tog_modal {
                                                        h(false, window, cx);
                                                    }
                                                })
                                        }),
                                ),
                        )
                        // Modal Body
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .p_4()
                                .bg(ThemeColors::BG_APP)
                                .overflow_scrollbar()
                                .child(if current_val.is_null() {
                                    div()
                                        .text_sm()
                                        .text_color(ThemeColors::TEXT_FAINT)
                                        .child("<NULL>")
                                } else {
                                    div()
                                        .text_xs()
                                        .font_family("JetBrains Mono")
                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                        .child(display_val_str)
                                }),
                        )
                        // Modal Footer
                        .child(
                            h_flex()
                                .w_full()
                                .p_3()
                                .px_4()
                                .justify_between()
                                .items_center()
                                .border_t_1()
                                .border_color(ThemeColors::BORDER)
                                .bg(ThemeColors::BG_SURFACE)
                                .child(div().text_xs().text_color(ThemeColors::TEXT_FAINT).child(
                                    format!("{char_count} characters · {line_count} line(s)"),
                                ))
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_2()
                                        .child({
                                            let on_tog_modal = on_tog_modal.clone();
                                            Button::new("modal_footer_close_btn")
                                                .ghost()
                                                .small()
                                                .label("Close")
                                                .on_click(move |_, window, cx| {
                                                    if let Some(ref h) = on_tog_modal {
                                                        h(false, window, cx);
                                                    }
                                                })
                                        })
                                        .child(
                                            Button::new("modal_footer_copy_btn")
                                                .small()
                                                .icon(IconName::Copy)
                                                .label("Copy Value")
                                                .on_click(move |_, window, cx| {
                                                    if let Some(ref h) = on_cp_val {
                                                        h(
                                                            col_name_cp.clone(),
                                                            val_cp.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    } else {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(
                                                                val_cp.clone(),
                                                            ),
                                                        );
                                                    }
                                                }),
                                        ),
                                ),
                        );

                    return div()
                        .size_full()
                        .relative()
                        .child(grid_content)
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(rgba(0x000000AA))
                                .justify_center()
                                .items_center()
                                .flex()
                                .child(modal_dialog),
                        )
                        .into_any_element();
                }
            }
        }

        grid_content.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_to_json_types() {
        let cols = vec![
            "id".to_string(),
            "name".to_string(),
            "active".to_string(),
            "payload".to_string(),
            "nullable".to_string(),
        ];
        let row = vec![
            QueryValue::Int(42),
            QueryValue::String("crab".to_string()),
            QueryValue::Bool(true),
            QueryValue::String(r#"{"key": "value"}"#.to_string()),
            QueryValue::Null,
        ];
        let json_str = row_to_json(&cols, &row);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["name"], "crab");
        assert_eq!(parsed["active"], true);
        assert_eq!(parsed["payload"]["key"], "value");
        assert!(parsed["nullable"].is_null());
    }

    #[test]
    fn test_row_to_tsv_newlines_sanitized() {
        let row = vec![
            QueryValue::Int(1),
            QueryValue::String("line 1\nline 2\r\nline 3".to_string()),
            QueryValue::String("tab\there".to_string()),
        ];
        let tsv = row_to_tsv(&row);
        assert!(!tsv.contains('\n'));
        assert!(!tsv.contains('\r'));
        let parts: Vec<&str> = tsv.split('\t').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "1");
        assert_eq!(parts[1], "line 1 line 2  line 3");
        assert_eq!(parts[2], "tab here");
    }

    #[test]
    fn test_format_inspector_value_plain_text() {
        let text = "First line\nSecond line\nThird line";
        let (val, is_json, chars, lines) = format_inspector_value(text, true);
        assert!(!is_json);
        assert_eq!(val, text);
        assert_eq!(lines, 3);
        assert_eq!(chars, text.chars().count());
    }

    #[test]
    fn test_format_inspector_value_json_pretty() {
        let raw_json = r#"{"name":"zqlcrab","features":["explain","inspector"]}"#;
        let (pretty, is_json, _chars, lines) = format_inspector_value(raw_json, true);
        assert!(is_json);
        assert!(lines > 1);
        assert!(pretty.contains("  \"name\": \"zqlcrab\""));

        let (raw, is_json_raw, _chars_raw, lines_raw) = format_inspector_value(raw_json, false);
        assert!(is_json_raw);
        assert_eq!(lines_raw, 1);
        assert_eq!(raw, raw_json);
    }

    #[test]
    fn test_grid_cell_coord_and_inserted_state() {
        let coord_existing = GridCellCoord::existing(2, 4);
        assert!(!coord_existing.is_inserted);
        assert_eq!(coord_existing.row_idx, 2);
        assert_eq!(coord_existing.col_idx, 4);

        let coord_inserted = GridCellCoord::inserted(0, 1);
        assert!(coord_inserted.is_inserted);
        assert_eq!(coord_inserted.row_idx, 0);
        assert_eq!(coord_inserted.col_idx, 1);
    }

    #[test]
    fn test_compute_grid_display_items_interleaving() {
        // Suppose page 0 has original rows [0, 1, 2]
        let page_slice = vec![0, 1, 2];

        // Inserted row 0 anchored after original row 0
        // Inserted row 1 anchored after inserted row 0 (temp_id 0)
        // Inserted row 2 anchored after original row 2
        // Inserted row 3 anchored at PageEnd(0)
        let inserted = vec![
            RowInsertion {
                temp_id: 0,
                values: vec![QueryValue::Int(10)],
                anchor: InsertAnchor::AfterRow(0),
            },
            RowInsertion {
                temp_id: 1,
                values: vec![QueryValue::Int(11)],
                anchor: InsertAnchor::AfterInserted(0),
            },
            RowInsertion {
                temp_id: 2,
                values: vec![QueryValue::Int(12)],
                anchor: InsertAnchor::AfterRow(2),
            },
            RowInsertion {
                temp_id: 3,
                values: vec![QueryValue::Int(13)],
                anchor: InsertAnchor::PageEnd(0),
            },
        ];

        let items = compute_grid_display_items(&page_slice, 0, 1, &inserted);
        assert_eq!(
            items,
            vec![
                GridDisplayItem::Original(0, 0),
                GridDisplayItem::Inserted(0), // anchored after orig 0
                GridDisplayItem::Inserted(1), // anchored after ins 0
                GridDisplayItem::Original(1, 1),
                GridDisplayItem::Original(2, 2),
                GridDisplayItem::Inserted(2), // anchored after orig 2
                GridDisplayItem::Inserted(3), // page end
            ]
        );
    }
}
