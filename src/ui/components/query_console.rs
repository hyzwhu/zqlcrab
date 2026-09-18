//! Interactive SQL query editor and execution console.

use crate::db::explain::ExplainPlan;
use crate::db::types::QueryResult;
use crate::ui::components::data_grid::DataGrid;
use crate::ui::components::explain_panel::{ExplainPanel, ExplainViewMode};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Textarea, TextareaState},
    resizable::{resizable_panel, v_resizable, ResizableState},
    tab::{Tab, TabBar},
};
use gpui_kit::gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsoleBottomTab {
    #[default]
    Results,
    Explain,
}

#[derive(IntoElement)]
pub struct QueryConsole {
    editor_state: Entity<TextareaState>,
    split_state: Entity<ResizableState>,
    query_result: Option<QueryResult>,
    query_error: Option<String>,
    explain_plan: Option<ExplainPlan>,
    explain_error: Option<String>,
    is_executing: bool,
    is_explaining: bool,
    bottom_tab: ConsoleBottomTab,
    explain_view: ExplainViewMode,
    connection_label: Option<String>,
    on_run: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_clear: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_format: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_explain: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_bottom_tab: Option<Rc<dyn Fn(ConsoleBottomTab, &mut Window, &mut App) + 'static>>,
    on_explain_view: Option<Rc<dyn Fn(ExplainViewMode, &mut Window, &mut App) + 'static>>,
}

impl QueryConsole {
    pub fn new(editor_state: &Entity<TextareaState>, split_state: &Entity<ResizableState>) -> Self {
        Self {
            editor_state: editor_state.clone(),
            split_state: split_state.clone(),
            query_result: None,
            query_error: None,
            explain_plan: None,
            explain_error: None,
            is_executing: false,
            is_explaining: false,
            bottom_tab: ConsoleBottomTab::Results,
            explain_view: ExplainViewMode::Tree,
            connection_label: None,
            on_run: None,
            on_clear: None,
            on_format: None,
            on_explain: None,
            on_bottom_tab: None,
            on_explain_view: None,
        }
    }

    pub fn result(mut self, result: Option<QueryResult>) -> Self {
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
}

impl RenderOnce for QueryConsole {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let busy = self.is_executing || self.is_explaining;

        let mut run_button = Button::new("run_query")
            .ghost()
            .small()
            .icon(IconName::Play)
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
            .icon(IconName::ListIndentIncrease)
            .label("Format")
            .tooltip("Format SQL (Shift+Alt+F)");
        if let Some(on_format) = self.on_format {
            format_button = format_button.on_click(move |_, window, cx| {
                on_format(window, cx);
            });
        }

        let mut explain_button = Button::new("explain_query")
            .ghost()
            .small()
            .icon(IconName::Activity)
            .label("Explain")
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
            .icon(IconName::Menu)
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
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(run_button)
                    .child(format_button)
                    .child(explain_button)
                    .child(
                        div()
                            .h(px(16.0))
                            .w(px(1.0))
                            .bg(ThemeColors::BORDER)
                            .mx_1(),
                    )
                    .child(clear_button),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .when_some(self.connection_label.clone(), |this, label| {
                        this.child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Icon::new(IconName::Database)
                                        .size(px(12.0))
                                        .text_color(ThemeColors::PRIMARY_LIGHT),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_MUTED)
                                        .child(label),
                                ),
                        )
                    }),
            );

        let error_banner = self.query_error.clone().map(|err| {
            h_flex()
                .p_2()
                .px_3()
                .gap_2()
                .items_center()
                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                .border_b_1()
                .border_color(ThemeColors::ERROR)
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size(px(14.0))
                        .text_color(ThemeColors::ERROR),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::ERROR)
                        .child(err),
                )
        });

        let editor_pane = div()
            .size_full()
            .min_h_0()
            .bg(ThemeColors::BG_APP)
            .child(
                Textarea::new(&self.editor_state)
                    .h_full()
                    .bg(ThemeColors::BG_APP)
                    .text_color(ThemeColors::TEXT_PRIMARY),
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
            .child(Tab::new().label("Results"))
            .child(Tab::new().label("Explain"));

        let stats = self.query_result.as_ref().map(|res| {
            format!(
                "Rows: {}  Time: {}ms",
                res.rows.len(),
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
            .child(bottom_tabs)
            .when_some(stats, |this, label| {
                this.child(
                    div()
                        .px_3()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child(label),
                )
            });

        let bottom_body = match self.bottom_tab {
            ConsoleBottomTab::Results => DataGrid::new(self.query_result).into_any_element(),
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

        v_flex()
            .size_full()
            .min_h_0()
            .bg(ThemeColors::BG_APP)
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
