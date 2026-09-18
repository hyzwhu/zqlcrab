//! Interactive SQL query editor and execution console.

use crate::db::types::QueryResult;
use crate::ui::components::data_grid::DataGrid;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Textarea, TextareaState},
};
use gpui_kit::gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct QueryConsole {
    editor_state: Entity<TextareaState>,
    query_result: Option<QueryResult>,
    query_error: Option<String>,
    is_executing: bool,
    on_run: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_clear: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl QueryConsole {
    pub fn new(editor_state: &Entity<TextareaState>) -> Self {
        Self {
            editor_state: editor_state.clone(),
            query_result: None,
            query_error: None,
            is_executing: false,
            on_run: None,
            on_clear: None,
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

    pub fn executing(mut self, is_executing: bool) -> Self {
        self.is_executing = is_executing;
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
}

impl RenderOnce for QueryConsole {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut run_button = Button::new("run_query")
            .primary()
            .small()
            .icon(IconName::Play)
            .label(if self.is_executing { "Executing..." } else { "Run (⌘↵)" });

        if let Some(on_run) = self.on_run {
            run_button = run_button.on_click(move |_, window, cx| {
                on_run(window, cx);
            });
        }

        let mut clear_button = Button::new("clear_query")
            .ghost()
            .small()
            .icon(IconName::Trash)
            .label("Clear");

        if let Some(on_clear) = self.on_clear {
            clear_button = clear_button.on_click(move |_, window, cx| {
                on_clear(window, cx);
            });
        }

        let toolbar = h_flex()
            .h(px(40.0))
            .w_full()
            .px_3()
            .items_center()
            .justify_between()
            .bg(ThemeColors::BG_SURFACE)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(run_button)
                    .child(clear_button),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .when_some(self.query_result.as_ref(), |this, res| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!(
                                    "{} rows in {}ms",
                                    res.rows.len(),
                                    res.execution_time_ms.unwrap_or(0)
                                )),
                        )
                    }),
            );

        // Error message banner
        let error_banner = self.query_error.map(|err| {
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

        // Editor state pane
        let editor_pane = div()
            .h(px(130.0))
            .w_full()
            .p_2()
            .bg(ThemeColors::BG_APP)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(Textarea::new(&self.editor_state).h(px(114.0)));

        // Results region
        let results_pane = div()
            .flex_1()
            .child(DataGrid::new(self.query_result));

        v_flex()
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(toolbar)
            .children(error_banner)
            .child(editor_pane)
            .child(results_pane)
    }
}
