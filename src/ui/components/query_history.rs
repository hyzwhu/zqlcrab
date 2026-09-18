//! Query history viewer and replay panel.

use crate::db::history::{QueryHistoryItem, QueryHistoryStatus};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct QueryHistoryView {
    items: Vec<QueryHistoryItem>,
    filter_keyword: String,
    on_load_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_run_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_copy_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_clear_history: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl QueryHistoryView {
    pub fn new(items: Vec<QueryHistoryItem>) -> Self {
        Self {
            items,
            filter_keyword: String::new(),
            on_load_query: None,
            on_run_query: None,
            on_copy_query: None,
            on_clear_history: None,
        }
    }

    pub fn filter_keyword(mut self, keyword: impl Into<String>) -> Self {
        self.filter_keyword = keyword.into();
        self
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
}

impl RenderOnce for QueryHistoryView {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let kw = self.filter_keyword.trim().to_lowercase();
        let filtered_items: Vec<&QueryHistoryItem> = if kw.is_empty() {
            self.items.iter().collect()
        } else {
            self.items
                .iter()
                .filter(|it| {
                    it.query_text.to_lowercase().contains(&kw)
                        || it
                            .connection_name
                            .as_deref()
                            .map(|cn| cn.to_lowercase().contains(&kw))
                            .unwrap_or(false)
                })
                .collect()
        };

        // Header toolbar
        let mut clear_btn = Button::new("clear_history_btn")
            .ghost()
            .xsmall()
            .icon(IconName::Delete)
            .tooltip("Clear All Query History")
            .child("Clear Log");

        if let Some(on_clear) = self.on_clear_history {
            clear_btn = clear_btn.on_click(move |_, window, cx| {
                on_clear(window, cx);
            });
        }

        let header = h_flex()
            .h(px(40.0))
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
                        Icon::new(IconName::RotateCw)
                            .size(px(14.0))
                            .text_color(ThemeColors::PRIMARY_BORDER),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("Execution History"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("{} entries", filtered_items.len())),
                    ),
            )
            .child(clear_btn);

        if filtered_items.is_empty() {
            return v_flex()
                .size_full()
                .bg(ThemeColors::BG_APP)
                .child(header)
                .child(
                    v_flex()
                        .flex_1()
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
                                .child("No query history recorded yet"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_FAINT)
                                .child("Executed SQL statements will automatically be recorded here for inspection and replay."),
                        ),
                );
        }

        let mut list_container = v_flex().gap_2().p_4();

        for (idx, item) in filtered_items.iter().enumerate() {
            let sql = item.query_text.clone();
            let sql_for_load = sql.clone();
            let sql_for_run = sql.clone();
            let sql_for_copy = sql.clone();

            let on_load = self.on_load_query.clone();
            let on_run = self.on_run_query.clone();
            let on_copy = self.on_copy_query.clone();

            let is_success = item.status == QueryHistoryStatus::Success;

            let status_badge = div()
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .when(is_success, |this| {
                    this.bg(ThemeColors::SUCCESS)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("OK")
                })
                .when(!is_success, |this| {
                    this.bg(ThemeColors::ERROR)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("ERR")
                });

            let mut load_btn = Button::new(gpui_kit::gpui::ElementId::NamedInteger("load_hist".into(), idx as u64))
                .ghost()
                .xsmall()
                .icon(IconName::FileText)
                .tooltip("Load into SQL Editor")
                .child("Load");

            if let Some(handler) = on_load {
                load_btn = load_btn.on_click(move |_, window, cx| {
                    handler(sql_for_load.clone(), window, cx);
                });
            }

            let mut run_btn = Button::new(gpui_kit::gpui::ElementId::NamedInteger("run_hist".into(), idx as u64))
                .primary()
                .xsmall()
                .icon(IconName::Play)
                .tooltip("Run Query Now")
                .child("Run");

            if let Some(handler) = on_run {
                run_btn = run_btn.on_click(move |_, window, cx| {
                    handler(sql_for_run.clone(), window, cx);
                });
            }

            let mut copy_btn = Button::new(gpui_kit::gpui::ElementId::NamedInteger("copy_hist".into(), idx as u64))
                .ghost()
                .xsmall()
                .icon(IconName::Copy)
                .tooltip("Copy SQL");

            if let Some(handler) = on_copy {
                copy_btn = copy_btn.on_click(move |_, window, cx| {
                    handler(sql_for_copy.clone(), window, cx);
                });
            }

            let timestamp_str = item.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string();

            let card = v_flex()
                .w_full()
                .p_3()
                .rounded_lg()
                .bg(ThemeColors::BG_SURFACE)
                .border_1()
                .border_color(ThemeColors::BORDER)
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
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_FAINT)
                                        .child(timestamp_str),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_MUTED)
                                        .child(format!("{} ms", item.execution_duration_ms)),
                                )
                                .when_some(item.rows_returned, |this, rows| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(format!("{rows} rows")),
                                    )
                                })
                                .when_some(item.rows_affected, |this, affected| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(format!("{affected} affected")),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .child(copy_btn)
                                .child(load_btn)
                                .child(run_btn),
                        ),
                )
                .child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(ThemeColors::BG_APP)
                        .text_xs()
                        .font_family("JetBrains Mono")
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
            .child(
                div()
                    .id("query_history_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list_container),
            )
    }
}
