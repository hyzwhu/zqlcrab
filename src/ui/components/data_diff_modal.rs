//! Visual Table Data Diff and Synchronization Wizard Modal Dialog.
//! Allows inspecting row-level data discrepancies (Insertions, Deletions, Modifications)
//! between two tables side-by-side, inspecting cell-level value differences, and generating
//! dialect-accurate DML synchronization scripts for PostgreSQL, MySQL, and SQLite.

use crate::db::data_diff::{
    DataDiffOptions, DataDiffReport, DataDiffStatus, DataSyncScript, RowDiff,
};
use crate::db::schema_diff::MigrationDirection;
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::gpui::{
    Anchor, App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px, rgba,
};
use std::rc::Rc;

/// Filter tabs for data discrepancy matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataDiffFilterTab {
    #[default]
    DifferencesOnly,
    All,
    AddedOnly,
    ModifiedOnly,
    DeletedOnly,
}

#[allow(clippy::type_complexity)]
#[derive(IntoElement)]
pub struct DataDiffModal {
    source_table: String,
    target_table: String,
    available_tables: Vec<String>,
    report: DataDiffReport,
    options: DataDiffOptions,
    sync_script: DataSyncScript,
    filter_tab: DataDiffFilterTab,
    copied: bool,
    language: AppLanguage,
    is_loading: bool,

    on_select_source_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_target_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_swap_tables: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_change_filter: Option<Rc<dyn Fn(DataDiffFilterTab, &mut Window, &mut App) + 'static>>,
    on_toggle_safe_mode: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_wrap_tx: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_change_direction: Option<Rc<dyn Fn(MigrationDirection, &mut Window, &mut App) + 'static>>,
    on_copy_sql: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_open_in_console: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_refresh: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl DataDiffModal {
    pub fn new(
        source_table: String,
        target_table: String,
        available_tables: Vec<String>,
        report: DataDiffReport,
        options: DataDiffOptions,
        sync_script: DataSyncScript,
        language: AppLanguage,
    ) -> Self {
        Self {
            source_table,
            target_table,
            available_tables,
            report,
            options,
            sync_script,
            filter_tab: DataDiffFilterTab::DifferencesOnly,
            copied: false,
            language,
            is_loading: false,
            on_select_source_table: None,
            on_select_target_table: None,
            on_swap_tables: None,
            on_change_filter: None,
            on_toggle_safe_mode: None,
            on_toggle_wrap_tx: None,
            on_change_direction: None,
            on_copy_sql: None,
            on_open_in_console: None,
            on_refresh: None,
            on_close: None,
        }
    }

    pub fn filter_tab(mut self, tab: DataDiffFilterTab) -> Self {
        self.filter_tab = tab;
        self
    }

    pub fn copied(mut self, copied: bool) -> Self {
        self.copied = copied;
        self
    }

    pub fn is_loading(mut self, loading: bool) -> Self {
        self.is_loading = loading;
        self
    }

    pub fn on_select_source_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_source_table = Some(Rc::new(handler));
        self
    }

    pub fn on_select_target_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_target_table = Some(Rc::new(handler));
        self
    }

    pub fn on_swap_tables<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_swap_tables = Some(Rc::new(handler));
        self
    }

    pub fn on_change_filter<F>(mut self, handler: F) -> Self
    where
        F: Fn(DataDiffFilterTab, &mut Window, &mut App) + 'static,
    {
        self.on_change_filter = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_safe_mode<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_safe_mode = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_wrap_tx<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_wrap_tx = Some(Rc::new(handler));
        self
    }

    pub fn on_change_direction<F>(mut self, handler: F) -> Self
    where
        F: Fn(MigrationDirection, &mut Window, &mut App) + 'static,
    {
        self.on_change_direction = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_sql<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_sql = Some(Rc::new(handler));
        self
    }

    pub fn on_open_in_console<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_open_in_console = Some(Rc::new(handler));
        self
    }

    pub fn on_refresh<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh = Some(Rc::new(handler));
        self
    }

    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    // --- Sub-renderers ---

    fn render_header(&self) -> impl IntoElement {
        let lang = self.language;
        let on_close = self.on_close.clone();

        h_flex()
            .w_full()
            .px_5()
            .py_3p5()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::ListOrdered)
                            .size(px(20.0))
                            .text_color(ThemeColors::PRIMARY),
                    )
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_base()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("data_diff.title", lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("data_diff.subtitle", lang)),
                            ),
                    ),
            )
            .child(
                Button::new("data_diff_close_x_btn")
                    .ghost()
                    .icon(IconName::Close)
                    .when_some(on_close, |btn, h| {
                        btn.on_click(move |_, window, cx| h(window, cx))
                    }),
            )
    }

    fn render_selectors(&self) -> impl IntoElement {
        let lang = self.language;
        let on_swap = self.on_swap_tables.clone();
        let on_refresh = self.on_refresh.clone();
        let on_src = self.on_select_source_table.clone();
        let on_tgt = self.on_select_target_table.clone();
        let tables = self.available_tables.clone();

        // Source Dropdown
        let src_tbl = if self.source_table.is_empty() {
            t("schema_diff.select_table", lang).to_string()
        } else {
            self.source_table.clone()
        };
        let src_tables_clone = tables.clone();
        let src_menu_btn = Button::new("data_diff_src_dropdown")
            .small()
            .outline()
            .icon(IconName::Database)
            .child(src_tbl)
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for tbl in &src_tables_clone {
                    let name = tbl.clone();
                    let cb = on_src.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = cb {
                                handler(name.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        // Target Dropdown
        let tgt_tbl = if self.target_table.is_empty() {
            t("schema_diff.select_table", lang).to_string()
        } else {
            self.target_table.clone()
        };
        let tgt_tables_clone = tables.clone();
        let tgt_menu_btn = Button::new("data_diff_tgt_dropdown")
            .small()
            .outline()
            .icon(IconName::Database)
            .child(tgt_tbl)
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for tbl in &tgt_tables_clone {
                    let name = tbl.clone();
                    let cb = on_tgt.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = cb {
                                handler(name.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        // Swap button
        let swap_btn = Button::new("data_diff_swap_tables_btn")
            .small()
            .ghost()
            .icon(IconName::ArrowRightLeft)
            .tooltip(t("schema_diff.swap_tooltip", lang))
            .when_some(on_swap, |btn, h| {
                btn.on_click(move |_, window, cx| h(window, cx))
            });

        // Refresh button
        let refresh_btn = Button::new("data_diff_refresh_btn")
            .small()
            .ghost()
            .icon(IconName::RefreshCw)
            .tooltip(t("common.refresh", lang))
            .when_some(on_refresh, |btn, h| {
                btn.on_click(move |_, window, cx| h(window, cx))
            });

        let summary = &self.report.summary;
        let keys_display = if self.report.key_columns.is_empty() {
            t("data_diff.no_keys", lang).to_string()
        } else {
            self.report.key_columns.join(", ")
        };

        h_flex()
            .w_full()
            .px_5()
            .py_2p5()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    // Source
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::PRIMARY)
                                    .child(format!("{}:", t("data_diff.source_table", lang))),
                            )
                            .child(src_menu_btn),
                    )
                    // Swap
                    .child(swap_btn)
                    // Target
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgba(0xa78bfafe))
                                    .child(format!("{}:", t("data_diff.target_table", lang))),
                            )
                            .child(tgt_menu_btn),
                    )
                    // Direction badge
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .text_xs()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(match self.options.direction {
                                MigrationDirection::SourceToTarget => "Source ➔ Target",
                                MigrationDirection::TargetToSource => "Target ➔ Source",
                            }),
                    )
                    // Key columns badge
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(rgba(0xf59e0b15))
                            .border_1()
                            .border_color(rgba(0xf59e0b40))
                            .child(
                                Icon::new(IconName::Key)
                                    .size(px(12.0))
                                    .text_color(rgba(0xfbbf24ff)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_color(rgba(0xfbbf24ff))
                                    .child(format!("PK: {}", keys_display)),
                            ),
                    )
                    .child(refresh_btn),
            )
            .child(
                // Metrics summary badges
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(rgba(0x10b98120))
                            .text_color(rgba(0x34d399ff))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child(format!(
                                "+ {} {}",
                                summary.added_count,
                                t("data_diff.added", lang)
                            )),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(rgba(0xf59e0b20))
                            .text_color(rgba(0xfbbf24ff))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child(format!(
                                "~ {} {}",
                                summary.modified_count,
                                t("data_diff.modified", lang)
                            )),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(rgba(0xef444420))
                            .text_color(rgba(0xf87171ff))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child(format!(
                                "- {} {}",
                                summary.deleted_count,
                                t("data_diff.deleted", lang)
                            )),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .text_xs()
                            .child(format!(
                                "= {} {}",
                                summary.identical_count,
                                t("data_diff.identical", lang)
                            )),
                    ),
            )
    }

    fn render_filter_tabs(&self) -> impl IntoElement {
        let lang = self.language;
        let cur_tab = self.filter_tab;
        let on_change = self.on_change_filter.clone();
        let summary = &self.report.summary;

        let tabs = [
            (
                DataDiffFilterTab::DifferencesOnly,
                "diff_tab_differences",
                format!(
                    "{}: {}",
                    t("schema_diff.tab_differences_only", lang),
                    summary.total_differences()
                ),
            ),
            (
                DataDiffFilterTab::All,
                "diff_tab_all",
                format!(
                    "{}: {}",
                    t("schema_diff.tab_all", lang),
                    self.report.rows.len()
                ),
            ),
            (
                DataDiffFilterTab::AddedOnly,
                "diff_tab_added",
                format!("{} (+{})", t("data_diff.added", lang), summary.added_count),
            ),
            (
                DataDiffFilterTab::ModifiedOnly,
                "diff_tab_modified",
                format!(
                    "{} (~{})",
                    t("data_diff.modified", lang),
                    summary.modified_count
                ),
            ),
            (
                DataDiffFilterTab::DeletedOnly,
                "diff_tab_deleted",
                format!(
                    "{} (-{})",
                    t("data_diff.deleted", lang),
                    summary.deleted_count
                ),
            ),
        ];

        let mut row = h_flex()
            .w_full()
            .px_5()
            .py_1p5()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE);

        for (tab, tab_id, label) in tabs {
            let is_sel = cur_tab == tab;
            let on_change = on_change.clone();
            let pill = div()
                .id(tab_id)
                .px_2p5()
                .py_1()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .bg(if is_sel {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BG_SURFACE_ACTIVE
                })
                .text_color(if is_sel {
                    ThemeColors::PRIMARY
                } else {
                    ThemeColors::TEXT_MUTED
                })
                .font_weight(if is_sel {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .hover(|s| {
                    s.bg(ThemeColors::PRIMARY_BORDER)
                        .text_color(ThemeColors::PRIMARY)
                })
                .child(label)
                .when_some(on_change, |el, h| {
                    el.on_click(move |_, window, cx| h(tab, window, cx))
                });

            row = row.child(pill);
        }

        row
    }

    fn render_row_diff_item(&self, row: &RowDiff, idx: usize) -> impl IntoElement {
        let status_badge = match row.status {
            DataDiffStatus::Added => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0x10b98120))
                .text_color(rgba(0x34d399ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("+ ADD"),
            DataDiffStatus::Deleted => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0xef444420))
                .text_color(rgba(0xf87171ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("- DROP"),
            DataDiffStatus::Modified => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0xf59e0b20))
                .text_color(rgba(0xfbbf24ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("~ MODIFY"),
            DataDiffStatus::Identical => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                .text_color(ThemeColors::TEXT_MUTED)
                .text_xs()
                .child("= SAME"),
        };

        let is_modified = row.status == DataDiffStatus::Modified;
        let mut cell_breakdown = v_flex().gap_1().w_full().pt_1p5();

        if is_modified {
            for cd in &row.cell_diffs {
                cell_breakdown = cell_breakdown.child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!("{}:", cd.column)),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(rgba(0xef444420))
                                .text_color(rgba(0xf87171ff))
                                .font_family(".AppleSystemUIFontMonospaced")
                                .child(if cd.source_display.is_empty() {
                                    "<empty>".to_string()
                                } else {
                                    cd.source_display.clone()
                                }),
                        )
                        .child(
                            Icon::new(IconName::ArrowRight)
                                .size(px(12.0))
                                .text_color(ThemeColors::TEXT_MUTED),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(rgba(0x10b98120))
                                .text_color(rgba(0x34d399ff))
                                .font_family(".AppleSystemUIFontMonospaced")
                                .child(if cd.target_display.is_empty() {
                                    "<empty>".to_string()
                                } else {
                                    cd.target_display.clone()
                                }),
                        ),
                );
            }
        }

        let item_id: &'static str = Box::leak(format!("data_diff_row_{}", idx).into_boxed_str());

        v_flex()
            .id(item_id)
            .w_full()
            .p_2p5()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex().items_center().gap_2().child(status_badge).child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_xs()
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(if row.key_summary.is_empty() {
                                    format!("Row #{}", idx + 1)
                                } else {
                                    row.key_summary.clone()
                                }),
                        ),
                    )
                    .child(if is_modified {
                        div()
                            .text_xs()
                            .text_color(rgba(0xfbbf24ff))
                            .child(format!("{} fields changed", row.cell_diffs.len()))
                    } else {
                        div()
                    }),
            )
            .when(is_modified, |this| this.child(cell_breakdown))
    }

    fn render_matrix(&self) -> impl IntoElement {
        let lang = self.language;
        let mut list = v_flex().gap_2().p_4();

        let filtered_rows: Vec<(usize, &RowDiff)> = self
            .report
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| match self.filter_tab {
                DataDiffFilterTab::DifferencesOnly => r.status.is_different(),
                DataDiffFilterTab::All => true,
                DataDiffFilterTab::AddedOnly => r.status == DataDiffStatus::Added,
                DataDiffFilterTab::ModifiedOnly => r.status == DataDiffStatus::Modified,
                DataDiffFilterTab::DeletedOnly => r.status == DataDiffStatus::Deleted,
            })
            .collect();

        if self.is_loading {
            return list.child(
                v_flex()
                    .w_full()
                    .py_12()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::RefreshCw)
                            .size(px(24.0))
                            .text_color(ThemeColors::PRIMARY),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("data_diff.loading", lang)),
                    ),
            );
        }

        if filtered_rows.is_empty() {
            let msg = if self.report.rows.is_empty() {
                t("data_diff.no_data", lang)
            } else {
                t("data_diff.no_differences_filtered", lang)
            };

            return list.child(
                v_flex()
                    .w_full()
                    .py_10()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Check)
                            .size(px(28.0))
                            .text_color(rgba(0x34d399ff)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(msg),
                    ),
            );
        }

        for (idx, row) in filtered_rows {
            list = list.child(self.render_row_diff_item(row, idx));
        }

        list
    }

    fn render_sync_preview_box(&self) -> impl IntoElement {
        let lang = self.language;
        let script = &self.sync_script.sql;
        let cur_safe = self.options.safe_mode;
        let cur_tx = self.options.wrap_transaction;
        let cur_dir = self.options.direction;

        let on_safe_toggle = self.on_toggle_safe_mode.clone();
        let on_tx_toggle = self.on_toggle_wrap_tx.clone();
        let on_dir_change = self.on_change_direction.clone();

        v_flex()
            .w_full()
            .h_full()
            .border_l_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            // Options bar
            .child(
                h_flex()
                    .w_full()
                    .px_4()
                    .py_2()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!(
                                "{} ({} statements)",
                                t("data_diff.sync_preview", lang),
                                self.sync_script.total_statements
                            )),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            // Direction toggle
                            .child(
                                div()
                                    .id("data_diff_dir_toggle_pill")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(match cur_dir {
                                        MigrationDirection::SourceToTarget => "Source ➔ Target",
                                        MigrationDirection::TargetToSource => "Target ➔ Source",
                                    })
                                    .when_some(on_dir_change, |el, h| {
                                        let next = match cur_dir {
                                            MigrationDirection::SourceToTarget => {
                                                MigrationDirection::TargetToSource
                                            }
                                            MigrationDirection::TargetToSource => {
                                                MigrationDirection::SourceToTarget
                                            }
                                        };
                                        el.on_click(move |_, window, cx| h(next, window, cx))
                                    }),
                            )
                            // Safe mode toggle
                            .child(
                                div()
                                    .id("data_diff_safe_mode_pill")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(if cur_safe {
                                        rgba(0x10b98120)
                                    } else {
                                        rgba(0xef444420)
                                    })
                                    .text_color(if cur_safe {
                                        rgba(0x34d399ff)
                                    } else {
                                        rgba(0xf87171ff)
                                    })
                                    .child(if cur_safe {
                                        t("schema_diff.safe_mode_on", lang)
                                    } else {
                                        t("schema_diff.safe_mode_off", lang)
                                    })
                                    .when_some(on_safe_toggle, |el, h| {
                                        el.on_click(move |_, window, cx| h(!cur_safe, window, cx))
                                    }),
                            )
                            // Transaction toggle
                            .child(
                                div()
                                    .id("data_diff_tx_toggle_pill")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(if cur_tx {
                                        ThemeColors::PRIMARY_BORDER
                                    } else {
                                        ThemeColors::BG_SURFACE_ACTIVE
                                    })
                                    .text_color(if cur_tx {
                                        ThemeColors::PRIMARY
                                    } else {
                                        ThemeColors::TEXT_MUTED
                                    })
                                    .child(if cur_tx {
                                        t("schema_diff.tx_on", lang)
                                    } else {
                                        t("schema_diff.tx_off", lang)
                                    })
                                    .when_some(on_tx_toggle, |el, h| {
                                        el.on_click(move |_, window, cx| h(!cur_tx, window, cx))
                                    }),
                            ),
                    ),
            )
            // SQL script text
            .child(
                v_flex()
                    .id("data_diff_script_scroll")
                    .flex_1()
                    .w_full()
                    .p_3()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_xs()
                            .text_color(rgba(0xa78bfafe))
                            .child(script.clone()),
                    ),
            )
    }

    fn render_footer(&self) -> impl IntoElement {
        let lang = self.language;
        let script = self.sync_script.sql.clone();
        let on_copy = self.on_copy_sql.clone();
        let on_console = self.on_open_in_console.clone();
        let on_close = self.on_close.clone();
        let is_copied = self.copied;

        let script_for_copy = script.clone();
        let script_for_console = script.clone();

        h_flex()
            .w_full()
            .px_5()
            .py_3()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            .child(
                Button::new("data_diff_cancel_btn")
                    .ghost()
                    .label(t("common.close", lang))
                    .when_some(on_close, |btn, h| {
                        btn.on_click(move |_, window, cx| h(window, cx))
                    }),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    // Copy button
                    .child(
                        Button::new("data_diff_copy_btn")
                            .secondary()
                            .icon(if is_copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .label(if is_copied {
                                t("common.copied", lang)
                            } else {
                                t("schema_diff.copy_script", lang)
                            })
                            .when_some(on_copy, |btn, h| {
                                btn.on_click(move |_, window, cx| {
                                    h(script_for_copy.clone(), window, cx)
                                })
                            }),
                    )
                    // Open in console button
                    .child(
                        Button::new("data_diff_console_btn")
                            .primary()
                            .icon(IconName::SquareArrowOutUpRight)
                            .label(t("schema_diff.open_in_console", lang))
                            .when_some(on_console, |btn, h| {
                                btn.on_click(move |_, window, cx| {
                                    h(script_for_console.clone(), window, cx)
                                })
                            }),
                    ),
            )
    }
}

impl RenderOnce for DataDiffModal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let dialog = v_flex()
            .w(px(1040.0))
            .h(px(720.0))
            .rounded_xl()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            .shadow_lg()
            .overflow_hidden()
            .child(self.render_header())
            .child(self.render_selectors())
            .child(self.render_filter_tabs())
            .child(
                h_flex()
                    .w_full()
                    .flex_1()
                    .overflow_hidden()
                    // Left side: comparison matrix
                    .child(
                        div()
                            .id("data_diff_matrix_scroll")
                            .w_3_5()
                            .h_full()
                            .overflow_y_scroll()
                            .child(self.render_matrix()),
                    )
                    // Right side: SQL preview
                    .child(div().w_2_5().h_full().child(self.render_sync_preview_box())),
            )
            .child(self.render_footer());

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000088))
            .items_center()
            .justify_center()
            .child(dialog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::DatabaseFamily;

    #[test]
    fn test_data_diff_modal_creation() {
        let report =
            DataDiffReport::empty("users".into(), "users_bak".into(), DatabaseFamily::Sqlite);
        let opts = DataDiffOptions::default();
        let script = DataSyncScript::default();

        let modal = DataDiffModal::new(
            "users".into(),
            "users_bak".into(),
            vec!["users".into(), "users_bak".into()],
            report,
            opts,
            script,
            AppLanguage::En,
        )
        .filter_tab(DataDiffFilterTab::DifferencesOnly)
        .copied(false);

        assert_eq!(modal.filter_tab, DataDiffFilterTab::DifferencesOnly);
        assert!(!modal.copied);
    }
}
