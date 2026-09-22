//! Interactive SQL query editor and execution console.

use crate::db::explain::ExplainPlan;
use crate::db::types::QueryResult;
use crate::settings::{AppLanguage, EditorSettings};
use crate::ui::components::data_grid::DataGrid;
use crate::ui::components::explain_panel::{ExplainPanel, ExplainViewMode};
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Editor, EditorState},
    resizable::{ResizableState, resizable_panel, v_resizable},
    tab::{Tab, TabBar},
};
use gpui_kit::gpui::{
    AnyElement, App, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
    px,
};
use std::rc::Rc;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryTabHeader {
    pub id: Uuid,
    pub title: String,
    pub connection_name: Option<String>,
    pub is_executing: bool,
    pub has_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsoleBottomTab {
    #[default]
    Results,
    Explain,
}

#[derive(IntoElement)]
pub struct QueryConsole {
    editor_state: Entity<EditorState>,
    split_state: Entity<ResizableState>,
    tabs: Vec<QueryTabHeader>,
    active_tab_id: Option<Uuid>,
    query_result: Option<Arc<QueryResult>>,
    query_error: Option<String>,
    explain_plan: Option<ExplainPlan>,
    explain_error: Option<String>,
    is_executing: bool,
    is_explaining: bool,
    bottom_tab: ConsoleBottomTab,
    explain_view: ExplainViewMode,
    connection_label: Option<String>,
    results_view: Option<AnyElement>,
    editor_settings: Option<EditorSettings>,
    language: AppLanguage,
    on_run: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_clear: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_format: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_explain: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_bottom_tab: Option<Rc<dyn Fn(ConsoleBottomTab, &mut Window, &mut App) + 'static>>,
    on_explain_view: Option<Rc<dyn Fn(ExplainViewMode, &mut Window, &mut App) + 'static>>,
    on_select_tab: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_close_tab: Option<Rc<dyn Fn(Uuid, &mut Window, &mut App) + 'static>>,
    on_new_tab: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl QueryConsole {
    pub fn new(editor_state: &Entity<EditorState>, split_state: &Entity<ResizableState>) -> Self {
        Self {
            editor_state: editor_state.clone(),
            split_state: split_state.clone(),
            tabs: Vec::new(),
            active_tab_id: None,
            query_result: None,
            query_error: None,
            explain_plan: None,
            explain_error: None,
            is_executing: false,
            is_explaining: false,
            bottom_tab: ConsoleBottomTab::Results,
            explain_view: ExplainViewMode::Tree,
            connection_label: None,
            results_view: None,
            editor_settings: None,
            language: AppLanguage::Auto,
            on_run: None,
            on_clear: None,
            on_format: None,
            on_explain: None,
            on_bottom_tab: None,
            on_explain_view: None,
            on_select_tab: None,
            on_close_tab: None,
            on_new_tab: None,
        }
    }

    pub fn tabs(mut self, tabs: Vec<QueryTabHeader>) -> Self {
        self.tabs = tabs;
        self
    }

    pub fn active_tab_id(mut self, id: Option<Uuid>) -> Self {
        self.active_tab_id = id;
        self
    }

    pub fn result(mut self, result: Option<Arc<QueryResult>>) -> Self {
        self.query_result = result;
        self
    }

    pub fn error(mut self, error: Option<String>) -> Self {
        self.query_error = error;
        self
    }

    pub fn explain_plan(mut self, plan: Option<ExplainPlan>) -> Self {
        self.explain_plan = plan;
        self
    }

    pub fn explain_error(mut self, error: Option<String>) -> Self {
        self.explain_error = error;
        self
    }

    pub fn executing(mut self, is_executing: bool) -> Self {
        self.is_executing = is_executing;
        self
    }

    pub fn explaining(mut self, is_explaining: bool) -> Self {
        self.is_explaining = is_explaining;
        self
    }

    pub fn bottom_tab(mut self, tab: ConsoleBottomTab) -> Self {
        self.bottom_tab = tab;
        self
    }

    pub fn explain_view(mut self, view: ExplainViewMode) -> Self {
        self.explain_view = view;
        self
    }

    pub fn connection_label(mut self, label: Option<String>) -> Self {
        self.connection_label = label;
        self
    }

    pub fn results_view(mut self, view: impl IntoElement) -> Self {
        self.results_view = Some(view.into_any_element());
        self
    }

    pub fn editor_settings(mut self, settings: EditorSettings) -> Self {
        self.editor_settings = Some(settings);
        self
    }

    pub fn language(mut self, lang: AppLanguage) -> Self {
        self.language = lang;
        self
    }

    pub fn on_run<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_run = Some(Rc::new(handler));
        self
    }

    pub fn on_clear<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_clear = Some(Rc::new(handler));
        self
    }

    pub fn on_format<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_format = Some(Rc::new(handler));
        self
    }

    pub fn on_explain<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_explain = Some(Rc::new(handler));
        self
    }

    pub fn on_bottom_tab<F>(mut self, handler: F) -> Self
    where
        F: Fn(ConsoleBottomTab, &mut Window, &mut App) + 'static,
    {
        self.on_bottom_tab = Some(Rc::new(handler));
        self
    }

    pub fn on_explain_view<F>(mut self, handler: F) -> Self
    where
        F: Fn(ExplainViewMode, &mut Window, &mut App) + 'static,
    {
        self.on_explain_view = Some(Rc::new(handler));
        self
    }

    pub fn on_select_tab<F>(mut self, handler: F) -> Self
    where
        F: Fn(Uuid, &mut Window, &mut App) + 'static,
    {
        self.on_select_tab = Some(Rc::new(handler));
        self
    }

    pub fn on_close_tab<F>(mut self, handler: F) -> Self
    where
        F: Fn(Uuid, &mut Window, &mut App) + 'static,
    {
        self.on_close_tab = Some(Rc::new(handler));
        self
    }

    pub fn on_new_tab<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_new_tab = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for QueryConsole {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        let busy = self.is_executing || self.is_explaining;
        let lang = self.language;

        let ed_font_size = self
            .editor_settings
            .as_ref()
            .map(|s| s.font_size)
            .unwrap_or(13.0);
        let ed_font_family = self
            .editor_settings
            .as_ref()
            .map(|s| s.font_family.clone())
            .unwrap_or_else(|| "JetBrains Mono".to_string());

        let run_label = if self.is_executing {
            t("console.running", lang)
        } else {
            t("console.run", lang)
        };

        let mut run_button = Button::new("run_query")
            .ghost()
            .small()
            .flex_shrink(1.0)
            .min_w(px(28.0))
            .overflow_hidden()
            .icon(IconName::Play)
            .label(run_label)
            .disabled(busy)
            .tooltip("Run query (⌘↵ / Ctrl+Enter)");
        if let Some(on_run) = self.on_run {
            run_button = run_button.on_click(move |_, window, cx| {
                on_run(window, cx);
            });
        }

        let mut format_button = Button::new("format_sql")
            .ghost()
            .small()
            .flex_shrink(1.0)
            .min_w(px(28.0))
            .overflow_hidden()
            .icon(IconName::ListIndentIncrease)
            .label(t("console.format", lang))
            .tooltip("Format SQL (Shift+Alt+F)");
        if let Some(on_format) = self.on_format {
            format_button = format_button.on_click(move |_, window, cx| {
                on_format(window, cx);
            });
        }

        let explain_label = if self.is_explaining {
            t("console.explaining", lang)
        } else {
            t("console.explain", lang)
        };

        let mut explain_button = Button::new("explain_query")
            .ghost()
            .small()
            .flex_shrink(1.0)
            .min_w(px(28.0))
            .overflow_hidden()
            .icon(IconName::Activity)
            .label(explain_label)
            .disabled(busy)
            .tooltip("Explain plan (⌘⇧E)");
        if let Some(on_explain) = self.on_explain {
            explain_button = explain_button.on_click(move |_, window, cx| {
                on_explain(window, cx);
            });
        }

        let mut clear_button = Button::new("clear_query")
            .ghost()
            .small()
            .flex_shrink(1.0)
            .min_w(px(28.0))
            .overflow_hidden()
            .icon(IconName::Trash)
            .label(t("console.clear", lang))
            .tooltip("Clear editor");
        if let Some(on_clear) = self.on_clear {
            clear_button = clear_button.on_click(move |_, window, cx| {
                on_clear(window, cx);
            });
        }

        let toolbar = h_flex()
            .h(px(36.0))
            .w_full()
            .px_2()
            .items_center()
            .justify_between()
            .bg(ThemeColors::BG_SURFACE)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .gap_2()
            .overflow_hidden()
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink(1.0)
                    .min_w_0()
                    .overflow_hidden()
                    .child(run_button)
                    .child(format_button)
                    .child(explain_button)
                    .child(
                        div()
                            .h(px(16.0))
                            .w(px(1.0))
                            .bg(ThemeColors::BORDER)
                            .mx_1()
                            .flex_shrink_0(),
                    )
                    .child(clear_button),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .flex_shrink(1.0)
                    .min_w_0()
                    .overflow_hidden()
                    .when_some(self.connection_label.clone(), |this, label| {
                        this.child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .flex_shrink(1.0)
                                .min_w_0()
                                .overflow_hidden()
                                .child(
                                    Icon::new(IconName::Database)
                                        .size(px(12.0))
                                        .text_color(ThemeColors::PRIMARY_LIGHT)
                                        .flex_shrink_0(),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_MUTED)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(label),
                                ),
                        )
                    }),
            );

        let error_banner = self.query_error.clone().map(|err| {
            let safe_err =
                crate::ui::components::connection_error_dialog::wrap_error_text(&err, 80);
            h_flex()
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .p_2()
                .px_3()
                .gap_2()
                .items_start()
                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                .border_b_1()
                .border_color(ThemeColors::ERROR)
                .child(
                    div().flex_shrink_0().pt_0p5().child(
                        Icon::new(IconName::TriangleAlert)
                            .size(px(14.0))
                            .text_color(ThemeColors::ERROR),
                    ),
                )
                .child(
                    v_flex()
                        .id("query_console_error_scroll")
                        .flex_1()
                        .min_w_0()
                        .max_h(px(80.0))
                        .overflow_y_scroll()
                        .whitespace_normal()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .text_color(ThemeColors::ERROR)
                        .child(safe_err),
                )
        });

        let editor_pane = div().size_full().min_h_0().bg(ThemeColors::BG_APP).child(
            Editor::new(&self.editor_state)
                .h_full()
                .bg(ThemeColors::BG_APP)
                .text_color(ThemeColors::TEXT_PRIMARY)
                .font_family(ed_font_family)
                .text_size(px(ed_font_size)),
        );

        let bottom_selected = match self.bottom_tab {
            ConsoleBottomTab::Results => 0,
            ConsoleBottomTab::Explain => 1,
        };
        let on_bottom = self.on_bottom_tab.clone();
        let bottom_tabs = TabBar::new("console-bottom-tabs")
            .small()
            .px_2()
            .selected_index(bottom_selected)
            .on_click(move |ix, window, cx| {
                if let Some(ref handler) = on_bottom {
                    handler(
                        if *ix == 1 {
                            ConsoleBottomTab::Explain
                        } else {
                            ConsoleBottomTab::Results
                        },
                        window,
                        cx,
                    );
                }
            })
            .child(Tab::new().label(t("console.results", lang)))
            .child(Tab::new().label(t("console.explain_tab", lang)));

        let stats = self.query_result.as_ref().map(|res| {
            format!(
                "{}: {}  {}: {}ms",
                t("console.rows", lang),
                res.rows.len(),
                t("console.time", lang),
                res.execution_time_ms.unwrap_or(0)
            )
        });

        let bottom_header = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .overflow_hidden()
            .gap_2()
            .child(
                div()
                    .flex_shrink(1.0)
                    .min_w_0()
                    .overflow_hidden()
                    .child(bottom_tabs),
            )
            .when_some(stats, |this, label| {
                this.child(
                    div()
                        .px_3()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_FAINT)
                        .flex_shrink_0()
                        .child(label),
                )
            });

        let bottom_body = match self.bottom_tab {
            ConsoleBottomTab::Results => {
                if let Some(view) = self.results_view {
                    view
                } else {
                    DataGrid::new(self.query_result).into_any_element()
                }
            }
            ConsoleBottomTab::Explain => ExplainPanel::new(self.explain_plan)
                .error(self.explain_error)
                .executing(self.is_explaining)
                .view(self.explain_view)
                .on_view_change({
                    let handler = self.on_explain_view.clone();
                    move |view, window, cx| {
                        if let Some(ref on_view) = handler {
                            on_view(view, window, cx);
                        }
                    }
                })
                .into_any_element(),
        };

        let bottom_pane = v_flex()
            .size_full()
            .min_h_0()
            .bg(ThemeColors::BG_APP)
            .child(bottom_header)
            .child(div().size_full().flex_1().min_h_0().child(bottom_body));

        let tab_bar = if !self.tabs.is_empty() {
            let mut tabs_row = h_flex()
                .h(px(34.0))
                .w_full()
                .px_2()
                .items_center()
                .bg(ThemeColors::BG_SURFACE)
                .border_b_1()
                .border_color(ThemeColors::BORDER)
                .gap_1()
                .overflow_x_hidden();

            for tab in &self.tabs {
                let is_active = self.active_tab_id == Some(tab.id);
                let tab_id = tab.id;
                let title = tab.title.clone();

                let mut tab_item = h_flex()
                    .id(format!("query_tab_item_{}", tab_id))
                    .h(px(26.0))
                    .items_center()
                    .gap_1()
                    .pl_2()
                    .pr_1()
                    .rounded_md()
                    .cursor_pointer();

                if is_active {
                    tab_item = tab_item
                        .bg(ThemeColors::BG_APP)
                        .border_1()
                        .border_color(ThemeColors::BORDER);
                } else {
                    tab_item = tab_item
                        .bg(ThemeColors::TRANSPARENT)
                        .border_1()
                        .border_color(ThemeColors::TRANSPARENT)
                        .hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER));
                }

                // Tab Icon
                let icon_name = if tab.is_executing {
                    IconName::RotateCw
                } else if tab.has_error {
                    IconName::CircleAlert
                } else {
                    IconName::Code
                };
                let icon_color = if tab.is_executing {
                    ThemeColors::WARNING
                } else if tab.has_error {
                    ThemeColors::ERROR
                } else if is_active {
                    ThemeColors::PRIMARY_LIGHT
                } else {
                    ThemeColors::TEXT_FAINT
                };
                tab_item =
                    tab_item.child(Icon::new(icon_name).size(px(13.0)).text_color(icon_color));

                // Tab Title
                let title_color = if is_active {
                    ThemeColors::TEXT_PRIMARY
                } else {
                    ThemeColors::TEXT_MUTED
                };
                let mut title_elem = div()
                    .text_xs()
                    .text_color(title_color)
                    .max_w(px(120.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(title);
                if is_active {
                    title_elem = title_elem.font_weight(FontWeight::SEMIBOLD);
                }
                tab_item = tab_item.child(title_elem);

                // Optional connection badge if bound
                if let Some(ref conn_name) = tab.connection_name {
                    tab_item = tab_item.child(
                        div()
                            .text_size(px(10.0))
                            .px_1()
                            .rounded_xs()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .text_color(if is_active {
                                ThemeColors::PRIMARY_LIGHT
                            } else {
                                ThemeColors::TEXT_FAINT
                            })
                            .max_w(px(70.0))
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(conn_name.clone()),
                    );
                }

                // Tab click to select
                if let Some(ref on_select) = self.on_select_tab {
                    let on_select = on_select.clone();
                    tab_item = tab_item.on_click(move |_, window, cx| {
                        on_select(tab_id, window, cx);
                    });
                }

                // Close button [×]
                if let Some(ref on_close) = self.on_close_tab {
                    let on_close = on_close.clone();
                    let close_btn = Button::new(format!("close_query_tab_{}", tab_id))
                        .ghost()
                        .xsmall()
                        .icon(IconName::X)
                        .tooltip("Close Tab (⌘W / Ctrl+W)")
                        .on_click(move |_, window, cx| {
                            on_close(tab_id, window, cx);
                        });
                    tab_item = tab_item.child(close_btn);
                }

                tabs_row = tabs_row.child(tab_item);
            }

            // New Tab button [+]
            if let Some(ref on_new) = self.on_new_tab {
                let on_new = on_new.clone();
                let new_btn = Button::new("new_query_tab_btn")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .tooltip("New Query Tab (⌘T / Ctrl+T)")
                    .on_click(move |_, window, cx| {
                        on_new(window, cx);
                    });
                tabs_row = tabs_row.child(new_btn);
            }

            Some(tabs_row)
        } else {
            None
        };

        v_flex()
            .size_full()
            .min_h_0()
            .bg(ThemeColors::BG_APP)
            .children(tab_bar)
            .child(toolbar)
            .children(error_banner)
            .child(
                div().flex_1().min_h_0().w_full().child(
                    v_resizable("query-console-split")
                        .with_state(&self.split_state)
                        .child(resizable_panel().child(editor_pane))
                        .child(
                            resizable_panel()
                                .size(px(280.0))
                                .size_range(px(160.0)..px(560.0))
                                .flex_none()
                                .child(bottom_pane),
                        ),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_tab_header_properties() {
        let id = Uuid::new_v4();
        let header = QueryTabHeader {
            id,
            title: "users.sql".to_string(),
            connection_name: Some("Local MySQL".to_string()),
            is_executing: true,
            has_error: false,
        };

        assert_eq!(header.id, id);
        assert_eq!(header.title, "users.sql");
        assert_eq!(header.connection_name.as_deref(), Some("Local MySQL"));
        assert!(header.is_executing);
        assert!(!header.has_error);
    }
}
