//! Visual Database Storage Analyzer & Table Maintenance Diagnostics Wizard Modal.
//!
//! Provides a two-tab interface for database inspection and automated maintenance:
//! 1. Storage Capacity Analysis: KPI summary cards, table size allocations, data vs index breakdown, and bloat metrics.
//! 2. Maintenance & Diagnostics Wizard: Dialect-specific maintenance operations (Optimize / Vacuum, Integrity Check, Analyze Statistics, Rebuild Indexes, Checksum), real-time progress, and live audit logs.

use crate::db::maintenance::{
    format_bytes, DatabaseStorageMetricsSummary, MaintenanceAction, MaintenanceLogEntry,
    MaintenanceOptions, MaintenanceProgress, MaintenanceStatus, MaintenanceSummary,
    TableStorageMetrics,
};
use crate::db::types::DatabaseFamily;
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::{ThemeColors, ThemeToken};
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    spinner::Spinner,
    Disableable as _, Icon, Sizable as _,
};
use gpui_kit::gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, px, rgba,
};
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaintenanceModalTab {
    #[default]
    Storage,
    Diagnostics,
}

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct MaintenanceModal {
    tab: MaintenanceModalTab,
    family: DatabaseFamily,
    database_name: String,
    summary: Option<DatabaseStorageMetricsSummary>,
    selected_tables: HashSet<String>,
    filter_query: String,
    selected_action: MaintenanceAction,
    options: MaintenanceOptions,
    is_loading_metrics: bool,
    is_executing: bool,
    progress: Option<MaintenanceProgress>,
    logs: Vec<MaintenanceLogEntry>,
    maintenance_summary: Option<MaintenanceSummary>,
    error_message: Option<String>,
    copied: bool,
    language: AppLanguage,

    // Callbacks
    on_tab_change: Option<Rc<dyn Fn(MaintenanceModalTab, &mut Window, &mut App) + 'static>>,
    on_refresh_metrics: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_filter_change: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_toggle_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_all_tables: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_select_action: Option<Rc<dyn Fn(MaintenanceAction, &mut Window, &mut App) + 'static>>,
    on_toggle_continue_on_error: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_full_vacuum: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_extended_check: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_start_maintenance: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_abort_maintenance: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_copy_logs: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_jump_to_maintenance: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl MaintenanceModal {
    pub fn new(
        tab: MaintenanceModalTab,
        family: DatabaseFamily,
        database_name: impl Into<String>,
        summary: Option<DatabaseStorageMetricsSummary>,
        selected_tables: HashSet<String>,
        filter_query: impl Into<String>,
        selected_action: MaintenanceAction,
        options: MaintenanceOptions,
        is_loading_metrics: bool,
        is_executing: bool,
        progress: Option<MaintenanceProgress>,
        logs: Vec<MaintenanceLogEntry>,
        maintenance_summary: Option<MaintenanceSummary>,
        error_message: Option<String>,
        copied: bool,
        language: AppLanguage,
    ) -> Self {
        Self {
            tab,
            family,
            database_name: database_name.into(),
            summary,
            selected_tables,
            filter_query: filter_query.into(),
            selected_action,
            options,
            is_loading_metrics,
            is_executing,
            progress,
            logs,
            maintenance_summary,
            error_message,
            copied,
            language,
            on_tab_change: None,
            on_refresh_metrics: None,
            on_filter_change: None,
            on_toggle_table: None,
            on_select_all_tables: None,
            on_select_action: None,
            on_toggle_continue_on_error: None,
            on_toggle_full_vacuum: None,
            on_toggle_extended_check: None,
            on_start_maintenance: None,
            on_abort_maintenance: None,
            on_copy_logs: None,
            on_jump_to_maintenance: None,
            on_close: None,
        }
    }

    pub fn on_tab_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(MaintenanceModalTab, &mut Window, &mut App) + 'static,
    {
        self.on_tab_change = Some(Rc::new(handler));
        self
    }

    pub fn on_refresh_metrics<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh_metrics = Some(Rc::new(handler));
        self
    }

    pub fn on_filter_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_filter_change = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_table = Some(Rc::new(handler));
        self
    }

    pub fn on_select_all_tables<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_select_all_tables = Some(Rc::new(handler));
        self
    }

    pub fn on_select_action<F>(mut self, handler: F) -> Self
    where
        F: Fn(MaintenanceAction, &mut Window, &mut App) + 'static,
    {
        self.on_select_action = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_continue_on_error<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_continue_on_error = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_full_vacuum<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_full_vacuum = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_extended_check<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_extended_check = Some(Rc::new(handler));
        self
    }

    pub fn on_start_maintenance<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_start_maintenance = Some(Rc::new(handler));
        self
    }

    pub fn on_abort_maintenance<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_abort_maintenance = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_logs<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_copy_logs = Some(Rc::new(handler));
        self
    }

    pub fn on_jump_to_maintenance<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_jump_to_maintenance = Some(Rc::new(handler));
        self
    }

    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    // -------------------------------------------------------------------------
    // Render: Header & Tab Navigation
    // -------------------------------------------------------------------------

    fn render_header(&self, lang: AppLanguage) -> impl IntoElement {
        let on_close_click = self.on_close.clone();

        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Gauge)
                                    .size(px(18.0))
                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("maintenance.title", lang)),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::BG_SURFACE_HOVER)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .text_size(px(10.0))
                                    .child(self.database_name.clone()),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::PRIMARY_BG)
                                    .text_color(ThemeColors::PRIMARY_LIGHT)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{:?}", self.family).to_uppercase()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("maintenance.subtitle", lang)),
                    ),
            )
            .child(
                Button::new("maintenance_close_btn")
                    .ghost()
                    .xsmall()
                    .icon(IconName::X)
                    .on_click(move |_, window, cx| {
                        if let Some(ref h) = on_close_click {
                            h(window, cx);
                        }
                    }),
            )
    }

    fn render_tab_nav(&self, lang: AppLanguage) -> impl IntoElement {
        let on_tab = self.on_tab_change.clone();

        let storage_active = self.tab == MaintenanceModalTab::Storage;
        let on_tab_storage = on_tab.clone();
        let mut storage_btn = Button::new("tab_storage_btn")
            .small()
            .icon(IconName::Database)
            .label(t("maintenance.tab_storage", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_tab_storage {
                    h(MaintenanceModalTab::Storage, window, cx);
                }
            });
        if storage_active {
            storage_btn = storage_btn.primary();
        } else {
            storage_btn = storage_btn.outline();
        }

        let diag_active = self.tab == MaintenanceModalTab::Diagnostics;
        let on_tab_diag = on_tab.clone();
        let mut diag_btn = Button::new("tab_diagnostics_btn")
            .small()
            .icon(IconName::Sparkles)
            .label(t("maintenance.tab_diagnostics", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_tab_diag {
                    h(MaintenanceModalTab::Diagnostics, window, cx);
                }
            });
        if diag_active {
            diag_btn = diag_btn.primary();
        } else {
            diag_btn = diag_btn.outline();
        }

        h_flex()
            .w_full()
            .px_4()
            .py_2()
            .gap_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(storage_btn)
            .child(diag_btn)
    }

    // -------------------------------------------------------------------------
    // Render: Tab 1 (Storage Capacity Analysis)
    // -------------------------------------------------------------------------

    fn render_storage_tab(&self, lang: AppLanguage) -> impl IntoElement {
        let summary = self.summary.clone().unwrap_or_default();
        let on_refresh = self.on_refresh_metrics.clone();
        let on_select_all = self.on_select_all_tables.clone();
        let on_jump = self.on_jump_to_maintenance.clone();

        // 1. KPI Cards (Total Size, Data Size, Index Size, Free Space, Tables, Rows)
        let kpi_row = h_flex()
            .w_full()
            .gap_2()
            .child(self.render_kpi_card(
                t("maintenance.kpi_total_size", lang),
                format_bytes(summary.total_size_bytes),
                ThemeColors::PRIMARY_LIGHT,
            ))
            .child(self.render_kpi_card(
                t("maintenance.kpi_data_size", lang),
                format_bytes(summary.total_data_bytes),
                ThemeColors::SUCCESS,
            ))
            .child(self.render_kpi_card(
                t("maintenance.kpi_index_size", lang),
                format_bytes(summary.total_index_bytes),
                ThemeColors::PRIMARY_BORDER,
            ))
            .child(self.render_kpi_card(
                t("maintenance.kpi_free_size", lang),
                format_bytes(summary.total_free_bytes),
                ThemeColors::WARNING,
            ))
            .child(self.render_kpi_card(
                t("maintenance.kpi_total_tables", lang),
                summary.total_tables.to_string(),
                ThemeColors::TEXT_PRIMARY,
            ))
            .child(self.render_kpi_card(
                t("maintenance.kpi_total_rows", lang),
                summary.total_rows.to_string(),
                ThemeColors::TEXT_MUTED,
            ));

        // 2. Toolbar & Filter
        let mut refresh_btn = Button::new("maintenance_refresh_btn")
            .outline()
            .small()
            .icon(IconName::RotateCw)
            .label(t("maintenance.btn_refresh", lang));
        if self.is_loading_metrics {
            refresh_btn = refresh_btn.disabled(true);
        } else if let Some(ref h) = on_refresh {
            let h = h.clone();
            refresh_btn = refresh_btn.on_click(move |_, window, cx| {
                h(window, cx);
            });
        }

        let on_sel_all = on_select_all.clone();
        let select_all_btn = Button::new("maintenance_sel_all_btn")
            .ghost()
            .small()
            .label(t("maintenance.btn_select_all", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_sel_all {
                    h(true, window, cx);
                }
            });

        let on_desel_all = on_select_all.clone();
        let deselect_all_btn = Button::new("maintenance_desel_all_btn")
            .ghost()
            .small()
            .label(t("maintenance.btn_deselect_all", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_desel_all {
                    h(false, window, cx);
                }
            });

        let selected_count = self.selected_tables.len();
        let mut maintain_btn = Button::new("maintenance_jump_btn")
            .primary()
            .small()
            .icon(IconName::Sparkles)
            .label(format!(
                "{} ({})",
                t("maintenance.btn_maintain_selected", lang),
                selected_count
            ));
        if selected_count == 0 {
            maintain_btn = maintain_btn.disabled(true);
        } else if let Some(ref h) = on_jump {
            let h = h.clone();
            maintain_btn = maintain_btn.on_click(move |_, window, cx| {
                h(window, cx);
            });
        }

        let toolbar = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!(
                                "{}: {}",
                                t("maintenance.inspected_tables", lang),
                                summary.tables.len()
                            )),
                    )
                    .child(select_all_btn)
                    .child(deselect_all_btn),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(refresh_btn)
                    .child(maintain_btn),
            );

        // 3. Table Breakdown Grid List
        let q = self.filter_query.to_lowercase();
        let filtered_tables: Vec<TableStorageMetrics> = summary
            .tables
            .iter()
            .filter(|t| q.is_empty() || t.table_name.to_lowercase().contains(&q))
            .cloned()
            .collect();
        let table_rows = self.render_table_breakdown_list(&filtered_tables, lang);

        let mut content = v_flex()
            .id("maintenance_storage_scroll")
            .size_full()
            .p_4()
            .gap_3()
            .overflow_y_scroll();

        if let Some(ref err) = self.error_message {
            content = content.child(
                div()
                    .p_2p5()
                    .rounded_md()
                    .bg(rgba(0xdc354515))
                    .border_1()
                    .border_color(ThemeColors::ERROR)
                    .text_xs()
                    .text_color(ThemeColors::ERROR)
                    .child(err.clone()),
            );
        }

        content
            .child(kpi_row)
            .child(toolbar)
            .child(table_rows)
    }

    fn render_kpi_card(&self, label: &str, value: String, accent_color: ThemeToken) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .p_2p5()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .rounded_md()
            .gap_1()
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(ThemeColors::TEXT_MUTED)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(accent_color)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(value),
            )
    }

    fn render_table_breakdown_list(
        &self,
        tables: &[TableStorageMetrics],
        lang: AppLanguage,
    ) -> impl IntoElement {
        let header = h_flex()
            .w_full()
            .px_3()
            .py_2()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .rounded_t_md()
            .text_size(px(11.0))
            .font_weight(FontWeight::BOLD)
            .text_color(ThemeColors::TEXT_MUTED)
            .child(div().w(px(32.0)).child(""))
            .child(div().w(px(180.0)).child(t("maintenance.col_table_name", lang)))
            .child(div().w(px(100.0)).child(t("maintenance.col_engine", lang)))
            .child(div().w(px(90.0)).child(t("maintenance.col_total_size", lang)))
            .child(div().w(px(160.0)).child(t("maintenance.col_ratio", lang)))
            .child(div().w(px(80.0)).child(t("maintenance.col_row_count", lang)))
            .child(div().flex_1().child(t("maintenance.col_free_bloat", lang)));

        let mut rows = v_flex()
            .w_full()
            .border_l_1()
            .border_r_1()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .rounded_b_md()
            .overflow_hidden();

        if tables.is_empty() {
            if self.is_loading_metrics {
                rows = rows.child(
                    h_flex()
                        .w_full()
                        .py_8()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(Spinner::new())
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(t("maintenance.loading_metrics", lang)),
                        ),
                );
            } else {
                rows = rows.child(
                    div()
                        .w_full()
                        .py_8()
                        .text_center()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(t("maintenance.no_tables_found", lang)),
                );
            }
        } else {
            for (idx, tbl) in tables.iter().enumerate() {
                let is_checked = self.selected_tables.contains(&tbl.table_name);
                let on_toggle = self.on_toggle_table.clone();
                let tbl_name = tbl.table_name.clone();

                let data_pct = (tbl.data_ratio() * 100.0).round() as u32;
                let idx_pct = (tbl.index_ratio() * 100.0).round() as u32;

                let row_bg = if idx % 2 == 1 {
                    ThemeColors::BG_SURFACE
                } else {
                    ThemeColors::BG_APP
                };

                let row = h_flex()
                    .w_full()
                    .px_3()
                    .py_2()
                    .bg(row_bg)
                    .items_center()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        div()
                            .w(px(32.0))
                            .child(
                                Button::new(format!("chk_{}", tbl.table_name))
                                    .ghost()
                                    .xsmall()
                                    .icon(if is_checked {
                                        IconName::Check
                                    } else {
                                        IconName::Circle
                                    })
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref h) = on_toggle {
                                            h(tbl_name.clone(), window, cx);
                                        }
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .w(px(180.0))
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(IconName::Table)
                                    .size(px(13.0))
                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(tbl.table_name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_size(px(11.0))
                            .text_color(ThemeColors::TEXT_MUTED)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(tbl.engine.clone().unwrap_or_else(|| "-".to_string())),
                    )
                    .child(
                        div()
                            .w(px(90.0))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(format_bytes(tbl.total_size_bytes)),
                    )
                    .child(
                        // Dual-color progress bar showing data vs index allocation
                        h_flex()
                            .w(px(160.0))
                            .items_center()
                            .gap_2()
                            .child(
                                h_flex()
                                    .w(px(90.0))
                                    .h(px(6.0))
                                    .rounded_full()
                                    .overflow_hidden()
                                    .bg(ThemeColors::BG_SURFACE_HOVER)
                                    .child(
                                        div()
                                            .h_full()
                                            .w(px(90.0 * tbl.data_ratio()))
                                            .bg(ThemeColors::SUCCESS),
                                    )
                                    .child(
                                        div()
                                            .h_full()
                                            .w(px(90.0 * tbl.index_ratio()))
                                            .bg(ThemeColors::PRIMARY_BORDER),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!("{data_pct}% / {idx_pct}%")),
                            ),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(tbl.row_count.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(if tbl.free_size_bytes > 0 {
                                ThemeColors::WARNING
                            } else {
                                ThemeColors::TEXT_MUTED
                            })
                            .child(format_bytes(tbl.free_size_bytes)),
                    );

                rows = rows.child(row);
            }
        }

        v_flex().w_full().child(header).child(rows)
    }

    // -------------------------------------------------------------------------
    // Render: Tab 2 (Maintenance & Diagnostics Wizard)
    // -------------------------------------------------------------------------

    fn render_diagnostics_tab(&self, lang: AppLanguage) -> impl IntoElement {
        let on_action = self.on_select_action.clone();
        let on_run = self.on_start_maintenance.clone();
        let on_abort = self.on_abort_maintenance.clone();
        let on_copy = self.on_copy_logs.clone();

        // 1. Action selector cards
        let actions = [
            (
                MaintenanceAction::Optimize,
                t("maintenance.action_optimize_title", lang),
                t("maintenance.action_optimize_desc", lang),
                IconName::Database,
            ),
            (
                MaintenanceAction::CheckIntegrity,
                t("maintenance.action_check_title", lang),
                t("maintenance.action_check_desc", lang),
                IconName::Shield,
            ),
            (
                MaintenanceAction::Analyze,
                t("maintenance.action_analyze_title", lang),
                t("maintenance.action_analyze_desc", lang),
                IconName::Gauge,
            ),
            (
                MaintenanceAction::RebuildIndex,
                t("maintenance.action_reindex_title", lang),
                t("maintenance.action_reindex_desc", lang),
                IconName::RotateCw,
            ),
            (
                MaintenanceAction::Checksum,
                t("maintenance.action_checksum_title", lang),
                t("maintenance.action_checksum_desc", lang),
                IconName::Check,
            ),
        ];

        let mut action_grid = h_flex().w_full().gap_2();
        for (action, title, desc, icon) in actions {
            let is_sel = self.selected_action == action;
            let on_act = on_action.clone();
            let hint = action.command_hint(self.family);

            let card = v_flex()
                .id(ElementId::Name(format!("act_card_{}", action.id()).into()))
                .flex_1()
                .p_2p5()
                .bg(if is_sel {
                    ThemeColors::BG_SURFACE_HOVER
                } else {
                    ThemeColors::BG_SURFACE
                })
                .border_1()
                .border_color(if is_sel {
                    ThemeColors::PRIMARY_LIGHT
                } else {
                    ThemeColors::BORDER
                })
                .rounded_md()
                .cursor_pointer()
                .gap_1()
                .on_click(move |_, window, cx| {
                    if let Some(ref h) = on_act {
                        h(action, window, cx);
                    }
                })
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    Icon::new(icon)
                                        .size(px(14.0))
                                        .text_color(if is_sel {
                                            ThemeColors::PRIMARY_LIGHT
                                        } else {
                                            ThemeColors::TEXT_MUTED
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if is_sel {
                                            ThemeColors::PRIMARY_LIGHT
                                        } else {
                                            ThemeColors::TEXT_PRIMARY
                                        })
                                        .child(title),
                                ),
                        )
                        .child(
                            div()
                                .px_1()
                                .py_0p5()
                                .rounded_sm()
                                .bg(ThemeColors::PRIMARY_BG)
                                .text_color(ThemeColors::PRIMARY_LIGHT)
                                .text_size(px(9.0))
                                .child(hint),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ThemeColors::TEXT_MUTED)
                        .line_height(px(14.0))
                        .child(desc),
                );

            action_grid = action_grid.child(card);
        }

        // 2. Options and Table target summary
        let target_count = self.selected_tables.len();
        let target_badge = if target_count == 0 {
            t("maintenance.all_tables", lang).to_string()
        } else {
            format!("{target_count} {}", t("maintenance.tables_selected", lang))
        };

        let on_cont = self.on_toggle_continue_on_error.clone();
        let cur_cont = self.options.continue_on_error;
        let cont_btn = Button::new("opt_cont_err_btn")
            .ghost()
            .small()
            .icon(if cur_cont {
                IconName::Check
            } else {
                IconName::Circle
            })
            .label(t("maintenance.opt_continue_on_error", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_cont {
                    h(!cur_cont, window, cx);
                }
            });

        let mut opt_row = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .p_2p5()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .rounded_md()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("maintenance.target_scope", lang)),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(ThemeColors::PRIMARY_BG)
                            .text_color(ThemeColors::PRIMARY_LIGHT)
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child(target_badge),
                    ),
            )
            .child(cont_btn);

        // Dialect-specific option: Full Vacuum (PG) or Extended Check (MySQL)
        if self.family == DatabaseFamily::Postgres {
            let on_fv = self.on_toggle_full_vacuum.clone();
            let cur_fv = self.options.full_vacuum;
            let fv_btn = Button::new("opt_full_vacuum_btn")
                .ghost()
                .small()
                .icon(if cur_fv {
                    IconName::Check
                } else {
                    IconName::Circle
                })
                .label(t("maintenance.opt_full_vacuum", lang))
                .on_click(move |_, window, cx| {
                    if let Some(ref h) = on_fv {
                        h(!cur_fv, window, cx);
                    }
                });
            opt_row = opt_row.child(fv_btn);
        } else if self.family == DatabaseFamily::MySql {
            let on_ec = self.on_toggle_extended_check.clone();
            let cur_ec = self.options.extended_check;
            let ec_btn = Button::new("opt_extended_check_btn")
                .ghost()
                .small()
                .icon(if cur_ec {
                    IconName::Check
                } else {
                    IconName::Circle
                })
                .label(t("maintenance.opt_extended_check", lang))
                .on_click(move |_, window, cx| {
                    if let Some(ref h) = on_ec {
                        h(!cur_ec, window, cx);
                    }
                });
            opt_row = opt_row.child(ec_btn);
        }

        // 3. Execution Action Bar
        let mut run_btn = Button::new("start_maintenance_btn")
            .primary()
            .small()
            .icon(IconName::Play)
            .label(t("maintenance.btn_run_diagnostics", lang));

        if self.is_executing {
            run_btn = run_btn.disabled(true);
        } else if let Some(ref h) = on_run {
            let h = h.clone();
            run_btn = run_btn.on_click(move |_, window, cx| {
                h(window, cx);
            });
        }

        let mut abort_btn = Button::new("abort_maintenance_btn")
            .outline()
            .small()
            .icon(IconName::X)
            .label(t("maintenance.btn_abort", lang));

        if !self.is_executing {
            abort_btn = abort_btn.disabled(true);
        } else if let Some(ref h) = on_abort {
            let h = h.clone();
            abort_btn = abort_btn.on_click(move |_, window, cx| {
                h(window, cx);
            });
        }

        let copy_btn = Button::new("copy_maintenance_logs_btn")
            .outline()
            .small()
            .icon(if self.copied {
                IconName::Check
            } else {
                IconName::Copy
            })
            .label(if self.copied {
                t("maintenance.btn_copied", lang)
            } else {
                t("maintenance.btn_copy_logs", lang)
            })
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_copy {
                    h(window, cx);
                }
            });

        let action_bar = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(run_btn)
                    .child(abort_btn),
            )
            .child(copy_btn);

        // 4. Progress bar (if executing or completed)
        let progress_section = if let Some(ref prog) = self.progress {
            let pct = (prog.percentage * 100.0).clamp(0.0, 100.0) as u32;
            v_flex()
                .w_full()
                .gap_1()
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .text_xs()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::PRIMARY_LIGHT)
                                .child(format!(
                                    "{} ({}/{}) - {}",
                                    t("maintenance.processing_table", lang),
                                    prog.current_index,
                                    prog.total_tables,
                                    prog.current_table
                                )),
                        )
                        .child(
                            div()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!("{pct}%")),
                        ),
                )
                .child(
                    h_flex()
                        .w_full()
                        .h(px(6.0))
                        .rounded_full()
                        .bg(ThemeColors::BG_SURFACE_HOVER)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(px(720.0 * prog.percentage))
                                .bg(ThemeColors::PRIMARY_LIGHT),
                        ),
                )
        } else {
            v_flex()
        };

        // 5. Live audit log stream
        let log_stream = self.render_audit_logs(lang);

        let mut content = v_flex()
            .size_full()
            .p_4()
            .gap_3();

        if let Some(ref err) = self.error_message {
            content = content.child(
                div()
                    .p_2p5()
                    .rounded_md()
                    .bg(rgba(0xdc354515))
                    .border_1()
                    .border_color(ThemeColors::ERROR)
                    .text_xs()
                    .text_color(ThemeColors::ERROR)
                    .child(err.clone()),
            );
        }

        content
            .child(action_grid)
            .child(opt_row)
            .child(action_bar)
            .child(progress_section)
            .child(log_stream)
    }

    fn render_audit_logs(&self, lang: AppLanguage) -> impl IntoElement {
        let mut log_container = v_flex()
            .id("maintenance_logs_scroll")
            .size_full()
            .min_h(px(220.0))
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .rounded_md()
            .p_3()
            .gap_1p5()
            .overflow_y_scroll();

        if self.logs.is_empty() {
            log_container = log_container.child(
                div()
                    .w_full()
                    .py_8()
                    .text_center()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(t("maintenance.no_logs_yet", lang)),
            );
        } else {
            for log in &self.logs {
                let badge_color = match log.status {
                    MaintenanceStatus::Success => ThemeColors::SUCCESS,
                    MaintenanceStatus::Warning => ThemeColors::WARNING,
                    MaintenanceStatus::Error => ThemeColors::ERROR,
                };

                let entry = h_flex()
                    .w_full()
                    .items_start()
                    .gap_2()
                    .text_size(px(11.0))
                    .line_height(px(16.0))
                    .child(
                        div()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(log.timestamp.clone()),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(badge_color)
                            .child(log.status.badge_text()),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(format!("[{}]", log.table_name)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(log.message.clone()),
                    )
                    .child(
                        div()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("{}ms", log.duration_ms)),
                    );

                log_container = log_container.child(entry);
            }
        }

        log_container
    }

    fn render_footer(&self, lang: AppLanguage) -> impl IntoElement {
        let on_close_click = self.on_close.clone();

        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_4()
            .py_2p5()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                if let Some(ref summary) = self.maintenance_summary {
                    h_flex()
                        .items_center()
                        .gap_3()
                        .text_xs()
                        .child(
                            div()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!(
                                    "{}: {}ms",
                                    t("maintenance.total_time", lang),
                                    summary.total_duration_ms
                                )),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::SUCCESS)
                                .child(format!(
                                    "{} {}",
                                    summary.succeeded,
                                    t("maintenance.passed", lang)
                                )),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if summary.failed > 0 {
                                    ThemeColors::ERROR
                                } else {
                                    ThemeColors::TEXT_MUTED
                                })
                                .child(format!(
                                    "{} {}",
                                    summary.failed,
                                    t("maintenance.failed", lang)
                                )),
                        )
                } else {
                    h_flex().child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("maintenance.ready_hint", lang)),
                    )
                },
            )
            .child(
                Button::new("maintenance_close_bottom_btn")
                    .outline()
                    .small()
                    .label(t("maintenance.btn_close", lang))
                    .on_click(move |_, window, cx| {
                        if let Some(ref h) = on_close_click {
                            h(window, cx);
                        }
                    }),
            )
    }
}

impl RenderOnce for MaintenanceModal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let lang = self.language;

        let header = self.render_header(lang);
        let tab_nav = self.render_tab_nav(lang);

        let body: AnyElement = match self.tab {
            MaintenanceModalTab::Storage => self.render_storage_tab(lang).into_any_element(),
            MaintenanceModalTab::Diagnostics => {
                self.render_diagnostics_tab(lang).into_any_element()
            }
        };

        let footer = self.render_footer(lang);

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000088))
            .items_center()
            .justify_center()
            .flex()
            .child(
                v_flex()
                    .w(px(820.0))
                    .max_h(px(640.0))
                    .bg(ThemeColors::BG_APP)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .rounded_lg()
                    .shadow_lg()
                    .overflow_hidden()
                    .child(header)
                    .child(tab_nav)
                    .child(body)
                    .child(footer),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maintenance_modal_initialization() {
        let modal = MaintenanceModal::new(
            MaintenanceModalTab::Storage,
            DatabaseFamily::Sqlite,
            "main.db",
            None,
            HashSet::new(),
            "",
            MaintenanceAction::Optimize,
            MaintenanceOptions::default(),
            false,
            false,
            None,
            Vec::new(),
            None,
            None,
            false,
            AppLanguage::En,
        );

        assert_eq!(modal.tab, MaintenanceModalTab::Storage);
        assert_eq!(modal.family, DatabaseFamily::Sqlite);
        assert_eq!(modal.database_name, "main.db");
        assert_eq!(modal.selected_action, MaintenanceAction::Optimize);
    }
}
