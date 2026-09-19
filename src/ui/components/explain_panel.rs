//! EXPLAIN plan viewer: Tree, Summary, and Raw.

use crate::db::explain::ExplainPlan;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    tab::{Tab, TabBar},
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExplainViewMode {
    #[default]
    Tree,
    Summary,
    Raw,
}

#[derive(IntoElement)]
pub struct ExplainPanel {
    plan: Option<ExplainPlan>,
    error: Option<String>,
    is_executing: bool,
    view: ExplainViewMode,
    on_view_change: Option<Rc<dyn Fn(ExplainViewMode, &mut Window, &mut App) + 'static>>,
}

impl ExplainPanel {
    pub fn new(plan: Option<ExplainPlan>) -> Self {
        Self {
            plan,
            error: None,
            is_executing: false,
            view: ExplainViewMode::Tree,
            on_view_change: None,
        }
    }

    pub fn error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }

    pub fn executing(mut self, executing: bool) -> Self {
        self.is_executing = executing;
        self
    }

    pub fn view(mut self, view: ExplainViewMode) -> Self {
        self.view = view;
        self
    }

    pub fn on_view_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(ExplainViewMode, &mut Window, &mut App) + 'static,
    {
        self.on_view_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ExplainPanel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        if self.is_executing {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("Running EXPLAIN…"),
                )
                .into_any_element();
        }

        if let Some(err) = self.error {
            return v_flex()
                .size_full()
                .p_3()
                .gap_2()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size(px(14.0))
                                .text_color(ThemeColors::ERROR),
                        )
                        .child(div().text_xs().text_color(ThemeColors::ERROR).child(err)),
                )
                .into_any_element();
        }

        let Some(plan) = self.plan else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Activity)
                        .size(px(28.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("No explain plan yet"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child("Run Explain from the editor toolbar to inspect the query plan."),
                )
                .into_any_element();
        };

        let view = self.view;
        let selected = match view {
            ExplainViewMode::Tree => 0,
            ExplainViewMode::Summary => 1,
            ExplainViewMode::Raw => 2,
        };
        let on_view = self.on_view_change.clone();
        let node_count = plan.node_count();
        let cost_label = plan
            .estimated_total_cost()
            .map(|c| format!("{c:.2}"))
            .unwrap_or_else(|| "—".to_string());
        let expensive = plan
            .most_expensive()
            .map(|n| n.display_name())
            .unwrap_or_else(|| "—".to_string());
        let dialect = plan.dialect_label();

        let header = h_flex()
            .w_full()
            .px_3()
            .py_1()
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
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("Explain Plan"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("{dialect} · {node_count} nodes")),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("Estimated total cost: {cost_label}")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child("|"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("Most expensive: {expensive}")),
                    ),
            );

        let tabs = TabBar::new("explain-view-tabs")
            .small()
            .px_2()
            .selected_index(selected)
            .on_click(move |ix, window, cx| {
                if let Some(ref handler) = on_view {
                    let next = match ix {
                        1 => ExplainViewMode::Summary,
                        2 => ExplainViewMode::Raw,
                        _ => ExplainViewMode::Tree,
                    };
                    handler(next, window, cx);
                }
            })
            .child(Tab::new().label("Tree"))
            .child(Tab::new().label("Summary"))
            .child(Tab::new().label("Raw"));

        let body = match view {
            ExplainViewMode::Tree => render_tree(&plan).into_any_element(),
            ExplainViewMode::Summary => render_summary(&plan).into_any_element(),
            ExplainViewMode::Raw => render_raw(&plan).into_any_element(),
        };

        v_flex()
            .size_full()
            .min_h_0()
            .bg(ThemeColors::BG_APP)
            .child(header)
            .child(tabs)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}

fn render_tree(plan: &ExplainPlan) -> impl IntoElement {
    let expensive_type = plan.most_expensive().map(|n| n.node_type.clone());
    let mut list = v_flex()
        .id("explain_tree_scroll")
        .size_full()
        .overflow_y_scroll()
        .p_2()
        .gap_1();

    for (depth, node) in plan.flatten() {
        let is_hot =
            expensive_type.as_deref() == Some(node.node_type.as_str()) && node.total_cost.is_some();
        let marker = if is_hot {
            ThemeColors::ERROR
        } else if node.total_cost.is_some() {
            ThemeColors::WARNING
        } else {
            ThemeColors::TEXT_FAINT
        };

        let mut row = h_flex()
            .w_full()
            .px_2()
            .py_1()
            .ml(px((depth as f32) * 16.0))
            .rounded_sm()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .items_center()
            .gap_2()
            .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(marker))
            .child(
                div()
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(node.node_type.clone()),
            );

        if let Some(rel) = node.relation.as_ref() {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(ThemeColors::PRIMARY_LIGHT)
                    .child(rel.clone()),
            );
        }

        if let Some(cost) = node.cost_label() {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(cost),
            );
        }

        if let Some(rows) = node.rows_label() {
            row = row.child(div().text_xs().text_color(ThemeColors::WARNING).child(rows));
        }

        if !node.details.is_empty() && node.relation.is_none() {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_FAINT)
                    .child(node.details.clone()),
            );
        }

        list = list.child(row);
    }

    list
}

fn render_summary(plan: &ExplainPlan) -> impl IntoElement {
    let mut body = TableBody::new();
    for (_, node) in plan.flatten() {
        body = body.child(
            TableRow::new()
                .child(TableCell::new().child(node.display_name()))
                .child(TableCell::new().child(node.relation.clone().unwrap_or_else(|| "-".into())))
                .child(TableCell::new().child(node.index.clone().unwrap_or_else(|| "-".into())))
                .child(
                    TableCell::new().child(
                        node.cost_label()
                            .unwrap_or_else(|| "-".into())
                            .replace("c:", ""),
                    ),
                )
                .child(
                    TableCell::new().child(
                        node.plan_rows
                            .map(|r| {
                                if r.fract() == 0.0 {
                                    format!("{}", r as i64)
                                } else {
                                    format!("{r:.2}")
                                }
                            })
                            .unwrap_or_else(|| "-".into()),
                    ),
                )
                .child(TableCell::new().child(if node.details.is_empty() {
                    "-".to_string()
                } else {
                    node.details.clone()
                })),
        );
    }

    div()
        .id("explain_summary_scroll")
        .size_full()
        .overflow_y_scroll()
        .child(
            Table::new()
                .w_full()
                .child(
                    TableHeader::new().child(
                        TableRow::new()
                            .child(TableHead::new().child("Node Type"))
                            .child(TableHead::new().child("Relation"))
                            .child(TableHead::new().child("Index"))
                            .child(TableHead::new().child("Cost"))
                            .child(TableHead::new().child("Rows"))
                            .child(TableHead::new().child("Details")),
                    ),
                )
                .child(body),
        )
}

fn render_raw(plan: &ExplainPlan) -> impl IntoElement {
    div()
        .id("explain_raw_scroll")
        .size_full()
        .overflow_y_scroll()
        .p_3()
        .bg(ThemeColors::BG_APP)
        .child(
            div()
                .font_family("JetBrains Mono")
                .text_xs()
                .text_color(ThemeColors::TEXT_MUTED)
                .whitespace_nowrap()
                .child(plan.raw.clone()),
        )
}
