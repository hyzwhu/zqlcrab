//! Query history viewer and replay panel with multi-dimensional filtering, KPI metrics, and export capabilities.

use crate::db::history::{
    HistoryExportFormat, HistoryStatusFilter, QueryHistoryItem, QueryHistoryStatus,
};
use crate::db::types::DatabaseFamily;
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
};
use gpui_kit::gpui::{
    App, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
    px, rgba,
};
use std::rc::Rc;

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct QueryHistoryView {
    items: Vec<QueryHistoryItem>,
    search_input: Entity<InputState>,
    selected_status: HistoryStatusFilter,
    selected_dialect: Option<DatabaseFamily>,
    language: AppLanguage,
    on_load_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_run_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_copy_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_clear_history: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_toggle_favorite: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_delete_item: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_save_as_snippet: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_export: Option<Rc<dyn Fn(HistoryExportFormat, &mut Window, &mut App) + 'static>>,
    on_select_status: Option<Rc<dyn Fn(HistoryStatusFilter, &mut Window, &mut App) + 'static>>,
    on_select_dialect: Option<Rc<dyn Fn(Option<DatabaseFamily>, &mut Window, &mut App) + 'static>>,
}

impl QueryHistoryView {
    pub fn new(
        items: Vec<QueryHistoryItem>,
        search_input: &Entity<InputState>,
        selected_status: HistoryStatusFilter,
        selected_dialect: Option<DatabaseFamily>,
        language: AppLanguage,
    ) -> Self {
        Self {
            items,
            search_input: search_input.clone(),
            selected_status,
            selected_dialect,
            language,
            on_load_query: None,
            on_run_query: None,
            on_copy_query: None,
            on_clear_history: None,
            on_toggle_favorite: None,
            on_delete_item: None,
            on_save_as_snippet: None,
            on_export: None,
            on_select_status: None,
            on_select_dialect: None,
        }
    }

    pub fn on_load_query<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_load_query = Some(Rc::new(handler));
        self
    }

    pub fn on_run_query<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_run_query = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_query<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_query = Some(Rc::new(handler));
        self
    }

    pub fn on_clear_history<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_clear_history = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_favorite<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_favorite = Some(Rc::new(handler));
        self
    }

    pub fn on_delete_item<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_delete_item = Some(Rc::new(handler));
        self
    }

    pub fn on_save_as_snippet<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_save_as_snippet = Some(Rc::new(handler));
        self
    }

    pub fn on_export<F>(mut self, handler: F) -> Self
    where
        F: Fn(HistoryExportFormat, &mut Window, &mut App) + 'static,
    {
        self.on_export = Some(Rc::new(handler));
        self
    }

    pub fn on_select_status<F>(mut self, handler: F) -> Self
    where
        F: Fn(HistoryStatusFilter, &mut Window, &mut App) + 'static,
    {
        self.on_select_status = Some(Rc::new(handler));
        self
    }

    pub fn on_select_dialect<F>(mut self, handler: F) -> Self
    where
        F: Fn(Option<DatabaseFamily>, &mut Window, &mut App) + 'static,
    {
        self.on_select_dialect = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for QueryHistoryView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // 1. Calculate overall metrics
        let total_queries = self.items.len();
        let success_count = self
            .items
            .iter()
            .filter(|it| it.status == QueryHistoryStatus::Success)
            .count();
        let error_count = self
            .items
            .iter()
            .filter(|it| it.status == QueryHistoryStatus::Error)
            .count();
        let slow_count = self
            .items
            .iter()
            .filter(|it| it.execution_duration_ms > 500)
            .count();
        let starred_count = self.items.iter().filter(|it| it.is_favorite).count();
        let total_duration_ms: u64 = self.items.iter().map(|it| it.execution_duration_ms).sum();
        let success_rate = if total_queries > 0 {
            (success_count as f64 / total_queries as f64) * 100.0
        } else {
            100.0
        };

        // 2. Perform multi-dimensional filtering
        let search_kw = self.search_input.read(cx).value().trim().to_lowercase();
        let filtered_items: Vec<&QueryHistoryItem> = self
            .items
            .iter()
            .filter(|it| {
                // Keyword match
                let matches_kw = if search_kw.is_empty() {
                    true
                } else {
                    it.query_text.to_lowercase().contains(&search_kw)
                        || it
                            .connection_name
                            .as_deref()
                            .map(|cn| cn.to_lowercase().contains(&search_kw))
                            .unwrap_or(false)
                        || it
                            .error_message
                            .as_deref()
                            .map(|err| err.to_lowercase().contains(&search_kw))
                            .unwrap_or(false)
                };

                // Status match
                let matches_status = match self.selected_status {
                    HistoryStatusFilter::All => true,
                    HistoryStatusFilter::SuccessOnly => it.status == QueryHistoryStatus::Success,
                    HistoryStatusFilter::ErrorOnly => it.status == QueryHistoryStatus::Error,
                    HistoryStatusFilter::SlowOnly => it.execution_duration_ms > 500,
                    HistoryStatusFilter::StarredOnly => it.is_favorite,
                };

                // Dialect match
                let matches_dialect = match self.selected_dialect {
                    None => true,
                    Some(fam) => it.database_type.map(|t| t.family()) == Some(fam),
                };

                matches_kw && matches_status && matches_dialect
            })
            .collect();

        // 3. Header Toolbar
        let mut export_csv_btn = Button::new("hist_export_csv")
            .ghost()
            .xsmall()
            .icon(IconName::Download)
            .tooltip(t("history.export_csv", self.language))
            .child("CSV");
        if let Some(handler) = self.on_export.clone() {
            export_csv_btn = export_csv_btn.on_click(move |_, window, cx| {
                handler(HistoryExportFormat::Csv, window, cx);
            });
        }

        let mut export_json_btn = Button::new("hist_export_json")
            .ghost()
            .xsmall()
            .icon(IconName::Code)
            .tooltip(t("history.export_json", self.language))
            .child("JSON");
        if let Some(handler) = self.on_export.clone() {
            export_json_btn = export_json_btn.on_click(move |_, window, cx| {
                handler(HistoryExportFormat::Json, window, cx);
            });
        }

        let mut export_sql_btn = Button::new("hist_export_sql")
            .ghost()
            .xsmall()
            .icon(IconName::FileText)
            .tooltip(t("history.export_sql", self.language))
            .child("SQL");
        if let Some(handler) = self.on_export {
            export_sql_btn = export_sql_btn.on_click(move |_, window, cx| {
                handler(HistoryExportFormat::Sql, window, cx);
            });
        }

        let mut clear_btn = Button::new("clear_history_btn")
            .ghost()
            .xsmall()
            .icon(IconName::Trash)
            .tooltip(t("history.clear_all", self.language))
            .child(t("history.clear_btn", self.language));
        if let Some(on_clear) = self.on_clear_history {
            clear_btn = clear_btn.on_click(move |_, window, cx| {
                on_clear(window, cx);
            });
        }

        let header = h_flex()
            .h(px(46.0))
            .w_full()
            .px_4()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Clock)
                            .size(px(16.0))
                            .text_color(ThemeColors::PRIMARY_BORDER),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("history.title", self.language)),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!(
                                "{} / {} {}",
                                filtered_items.len(),
                                total_queries,
                                t("history.items_suffix", self.language)
                            )),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(t("history.export_label", self.language)),
                    )
                    .child(export_csv_btn)
                    .child(export_json_btn)
                    .child(export_sql_btn)
                    .child(div().w(px(1.0)).h(px(16.0)).bg(ThemeColors::BORDER))
                    .child(clear_btn),
            );

        // 4. Metric KPI Strip
        let kpi_strip = h_flex()
            .w_full()
            .px_4()
            .py_2()
            .gap_3()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            // Total KPI
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("history.kpi_total", self.language)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(total_queries.to_string()),
                    ),
            )
            // Success Rate KPI
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("history.kpi_success_rate", self.language)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if success_rate >= 95.0 {
                                ThemeColors::SUCCESS
                            } else {
                                ThemeColors::WARNING
                            })
                            .child(format!("{:.1}%", success_rate)),
                    ),
            )
            // Slow Queries KPI
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("history.kpi_slow_queries", self.language)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if slow_count > 0 {
                                ThemeColors::WARNING
                            } else {
                                ThemeColors::TEXT_PRIMARY
                            })
                            .child(slow_count.to_string()),
                    ),
            )
            // Total Duration KPI
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("history.kpi_total_duration", self.language)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::PRIMARY_BORDER)
                            .child(format!("{total_duration_ms} ms")),
                    ),
            );

        // 5. Filter Bar (Search + Status + Dialect)
        let search_bar = Input::new(&self.search_input)
            .cleanable(true)
            .prefix(Icon::new(IconName::Search).size(px(14.0)));

        // Status filter pills
        let status_options = [
            (
                HistoryStatusFilter::All,
                t("history.filter_all", self.language),
                total_queries,
            ),
            (
                HistoryStatusFilter::SuccessOnly,
                t("history.filter_success", self.language),
                success_count,
            ),
            (
                HistoryStatusFilter::ErrorOnly,
                t("history.filter_error", self.language),
                error_count,
            ),
            (
                HistoryStatusFilter::SlowOnly,
                t("history.filter_slow", self.language),
                slow_count,
            ),
            (
                HistoryStatusFilter::StarredOnly,
                t("history.filter_starred", self.language),
                starred_count,
            ),
        ];

        let mut status_pills = h_flex().items_center().gap_1().flex_shrink_0();
        for (st, label, count) in status_options {
            let is_selected = self.selected_status == st;
            let handler = self.on_select_status.clone();
            let pill = div()
                .id(ElementId::Name(format!("hist_st_{:?}", st).into()))
                .flex_shrink_0()
                .px_2()
                .py_0p5()
                .rounded_full()
                .text_xs()
                .font_weight(if is_selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .cursor_pointer()
                .when(is_selected, |this| {
                    this.bg(ThemeColors::PRIMARY)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                })
                .when(!is_selected, |this| {
                    this.bg(ThemeColors::BG_SURFACE)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .hover(|h| {
                            h.bg(ThemeColors::BG_SURFACE_ACTIVE)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                        })
                })
                .child(format!("{label} ({count})"))
                .when_some(handler, |this, h| {
                    this.on_click(move |_, window, cx| {
                        h(st, window, cx);
                    })
                });

            status_pills = status_pills.child(pill);
        }

        // Dialect filter pills
        let dialect_options = [
            (None, t("history.dialect_all", self.language)),
            (Some(DatabaseFamily::Postgres), "PostgreSQL"),
            (Some(DatabaseFamily::MySql), "MySQL"),
            (Some(DatabaseFamily::Sqlite), "SQLite"),
        ];

        let mut dialect_pills = h_flex().items_center().gap_1().flex_shrink_0();
        for (opt_dialect, label) in dialect_options {
            let is_selected = self.selected_dialect == opt_dialect;
            let handler = self.on_select_dialect.clone();
            let pill = div()
                .id(ElementId::Name(format!("hist_dia_{}", label).into()))
                .flex_shrink_0()
                .px_2()
                .py_0p5()
                .rounded_full()
                .text_xs()
                .font_weight(if is_selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .cursor_pointer()
                .when(is_selected, |this| {
                    this.bg(ThemeColors::PRIMARY_BORDER)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                })
                .when(!is_selected, |this| {
                    this.bg(ThemeColors::BG_SURFACE)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .hover(|h| {
                            h.bg(ThemeColors::BG_SURFACE_ACTIVE)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                        })
                })
                .child(label)
                .when_some(handler, |this, h| {
                    this.on_click(move |_, window, cx| {
                        h(opt_dialect, window, cx);
                    })
                });

            dialect_pills = dialect_pills.child(pill);
        }

        let filter_bar = v_flex()
            .w_full()
            .px_4()
            .py_2()
            .gap_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().child(search_bar)),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(status_pills)
                    .child(dialect_pills),
            );

        // 6. List content
        if filtered_items.is_empty() {
            let empty_hint = if !search_kw.is_empty()
                || self.selected_status != HistoryStatusFilter::All
                || self.selected_dialect.is_some()
            {
                t("history.no_matching", self.language)
            } else {
                t("history.empty_hint", self.language)
            };

            return v_flex()
                .size_full()
                .bg(ThemeColors::BG_APP)
                .child(header)
                .child(kpi_strip)
                .child(filter_bar)
                .child(
                    v_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::RotateCw)
                                .size(px(36.0))
                                .text_color(ThemeColors::TEXT_FAINT),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(empty_hint),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_FAINT)
                                .child(t("history.empty_desc", self.language)),
                        ),
                );
        }

        let mut list_container = v_flex().gap_2().p_4();

        for (idx, item) in filtered_items.into_iter().enumerate() {
            let sql_for_load = item.query_text.clone();
            let sql_for_run = item.query_text.clone();
            let sql_for_copy = item.query_text.clone();
            let sql_for_snippet = item.query_text.clone();
            let item_id_fav = item.id.clone();
            let item_id_del = item.id.clone();

            let on_load = self.on_load_query.clone();
            let on_run = self.on_run_query.clone();
            let on_copy = self.on_copy_query.clone();
            let on_snippet = self.on_save_as_snippet.clone();
            let on_fav = self.on_toggle_favorite.clone();
            let on_del = self.on_delete_item.clone();

            let status_badge = div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .when(item.status == QueryHistoryStatus::Success, |this| {
                    this.bg(rgba(0x22c55e20))
                        .text_color(ThemeColors::SUCCESS)
                        .child(t("history.badge_success", self.language))
                })
                .when(item.status == QueryHistoryStatus::Error, |this| {
                    this.bg(rgba(0xef444420))
                        .text_color(ThemeColors::ERROR)
                        .child(t("history.badge_error", self.language))
                });

            // Star / Favorite toggle button
            let mut fav_btn = Button::new(ElementId::NamedInteger("fav_hist".into(), idx as u64))
                .ghost()
                .xsmall()
                .icon(if item.is_favorite {
                    IconName::StarFill
                } else {
                    IconName::Star
                })
                .tooltip(if item.is_favorite {
                    t("history.unstar_item", self.language)
                } else {
                    t("history.star_item", self.language)
                });
            if item.is_favorite {
                fav_btn = fav_btn.text_color(ThemeColors::WARNING);
            }
            if let Some(handler) = on_fav {
                fav_btn = fav_btn.on_click(move |_, window, cx| {
                    handler(item_id_fav.clone(), window, cx);
                });
            }

            // Load button
            let mut load_btn = Button::new(ElementId::NamedInteger("load_hist".into(), idx as u64))
                .ghost()
                .xsmall()
                .icon(IconName::Terminal)
                .tooltip(t("history.load_tooltip", self.language))
                .child(t("history.load_btn", self.language));
            if let Some(handler) = on_load {
                load_btn = load_btn.on_click(move |_, window, cx| {
                    handler(sql_for_load.clone(), window, cx);
                });
            }

            // Run button
            let mut run_btn = Button::new(ElementId::NamedInteger("run_hist".into(), idx as u64))
                .ghost()
                .xsmall()
                .icon(IconName::Play)
                .tooltip(t("history.run_tooltip", self.language))
                .child(t("history.run_btn", self.language));
            if let Some(handler) = on_run {
                run_btn = run_btn.on_click(move |_, window, cx| {
                    handler(sql_for_run.clone(), window, cx);
                });
            }

            // Copy button
            let mut copy_btn = Button::new(ElementId::NamedInteger("copy_hist".into(), idx as u64))
                .ghost()
                .xsmall()
                .icon(IconName::Copy)
                .tooltip(t("history.copy_tooltip", self.language));
            if let Some(handler) = on_copy {
                copy_btn = copy_btn.on_click(move |_, window, cx| {
                    handler(sql_for_copy.clone(), window, cx);
                });
            }

            // Snippet button
            let mut snippet_btn =
                Button::new(ElementId::NamedInteger("snip_hist".into(), idx as u64))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Code)
                    .tooltip(t("history.save_snippet", self.language));
            if let Some(handler) = on_snippet {
                snippet_btn = snippet_btn.on_click(move |_, window, cx| {
                    handler(sql_for_snippet.clone(), window, cx);
                });
            }

            // Delete button
            let mut del_btn = Button::new(ElementId::NamedInteger("del_hist".into(), idx as u64))
                .ghost()
                .xsmall()
                .icon(IconName::Trash)
                .tooltip(t("history.delete_item", self.language));
            if let Some(handler) = on_del {
                del_btn = del_btn.on_click(move |_, window, cx| {
                    handler(item_id_del.clone(), window, cx);
                });
            }

            let timestamp_str = item.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string();

            let card = v_flex()
                .w_full()
                .p_3()
                .rounded_lg()
                .bg(ThemeColors::BG_SURFACE)
                .border_1()
                .border_color(if item.is_favorite {
                    ThemeColors::WARNING
                } else {
                    ThemeColors::BORDER
                })
                .gap_2()
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(fav_btn)
                                .child(status_badge)
                                .when_some(item.connection_name.as_ref(), |this, cn| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(ThemeColors::PRIMARY_BORDER)
                                            .child(cn.clone()),
                                    )
                                })
                                .when_some(item.database_type, |this, dt| {
                                    this.child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(dt.display_name()),
                                    )
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if item.execution_duration_ms > 500 {
                                            ThemeColors::WARNING
                                        } else {
                                            ThemeColors::TEXT_MUTED
                                        })
                                        .child(format!("{} ms", item.execution_duration_ms)),
                                )
                                .when_some(item.rows_returned, |this, r| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(format!("{r} rows")),
                                    )
                                })
                                .when_some(item.rows_affected, |this, a| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(format!("{a} affected")),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_FAINT)
                                        .child(timestamp_str),
                                )
                                .child(load_btn)
                                .child(run_btn)
                                .child(copy_btn)
                                .child(snippet_btn)
                                .child(del_btn),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .p_2()
                        .rounded_md()
                        .bg(ThemeColors::BG_APP)
                        .font_family("JetBrains Mono, monospace")
                        .text_xs()
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(item.query_text.clone()),
                )
                .when_some(item.error_message.as_ref(), |this, err| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::ERROR)
                            .child(format!("Error: {err}")),
                    )
                });

            list_container = list_container.child(card);
        }

        v_flex()
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(header)
            .child(kpi_strip)
            .child(filter_bar)
            .child(
                div()
                    .id("query_history_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list_container),
            )
    }
}
