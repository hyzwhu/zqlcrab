//! Visual SQL Query Profiler & Resource Timeline Panel.
//!
//! Renders:
//! 1. KPI Metric Summary Cards (Total Duration, Planning Time, Execution Time, Buffer Cache Hit Ratio, Bottleneck Stage).
//! 2. Interactive Waterfall & Gantt Timeline Visualization with microsecond markers and colored stage breakdown.
//! 3. Tabular Stage Breakdown with Category, Start Offset, Duration, Ratio, and CPU/IO Details.
//! 4. One-click Copy Formatted Execution Profile to Clipboard.

use crate::db::profile::{format_duration_us, ProfileCategory, QueryProfileReport};
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::{ThemeColors, ThemeToken};
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    button::Button,
    spinner::Spinner,
    Icon, Sizable as _,
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, px, rgba,
};
use std::rc::Rc;

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct ProfilePanel {
    report: Option<QueryProfileReport>,
    error: Option<String>,
    is_profiling: bool,
    copied: bool,
    language: AppLanguage,
    on_copy: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl ProfilePanel {
    pub fn new(report: Option<QueryProfileReport>) -> Self {
        Self {
            report,
            error: None,
            is_profiling: false,
            copied: false,
            language: AppLanguage::Auto,
            on_copy: None,
        }
    }

    pub fn error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }

    pub fn profiling(mut self, profiling: bool) -> Self {
        self.is_profiling = profiling;
        self
    }

    pub fn copied(mut self, copied: bool) -> Self {
        self.copied = copied;
        self
    }

    pub fn language(mut self, lang: AppLanguage) -> Self {
        self.language = lang;
        self
    }

    pub fn on_copy<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_copy = Some(Rc::new(handler));
        self
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

    fn stage_accent_color(category: ProfileCategory, pct: f64) -> ThemeToken {
        if pct >= 0.35 {
            ThemeColors::ERROR
        } else {
            match category {
                ProfileCategory::Compilation => ThemeColors::TEXT_MUTED,
                ProfileCategory::Optimization => ThemeColors::WARNING,
                ProfileCategory::Execution => ThemeColors::PRIMARY_LIGHT,
                ProfileCategory::Transmission => ThemeColors::SUCCESS,
                ProfileCategory::CleanUp => ThemeColors::PRIMARY_BORDER,
            }
        }
    }

    fn render_waterfall(&self, report: &QueryProfileReport) -> impl IntoElement {
        let total_us = report.metrics.total_duration_us.max(1);
        let max_scale_ms = total_us as f64 / 1000.0;

        // Time axis scale markers: 0%, 25%, 50%, 75%, 100%
        let time_axis = h_flex()
            .w_full()
            .justify_between()
            .px_3()
            .py_1()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .text_size(px(10.0))
            .text_color(ThemeColors::TEXT_FAINT)
            .child(div().child("0 ms"))
            .child(div().child(format!("{:.1} ms", max_scale_ms * 0.25)))
            .child(div().child(format!("{:.1} ms", max_scale_ms * 0.50)))
            .child(div().child(format!("{:.1} ms", max_scale_ms * 0.75)))
            .child(div().child(format!("{:.1} ms", max_scale_ms)));

        let mut timeline_bars = v_flex().w_full().gap_1p5().p_3();

        for stage in &report.stages {
            let offset_ratio = (stage.start_time_us as f64 / total_us as f64).clamp(0.0, 1.0);
            let width_ratio = (stage.duration_us as f64 / total_us as f64).clamp(0.01, 1.0);
            let accent = Self::stage_accent_color(stage.category, stage.percentage);

            let bar_row = h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(160.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(stage.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(18.0))
                        .rounded_sm()
                        .bg(ThemeColors::BG_SURFACE_HOVER)
                        .relative()
                        .overflow_hidden()
                        .child(
                            div()
                                .absolute()
                                .top(px(2.0))
                                .bottom(px(2.0))
                                .left(px((offset_ratio * 380.0) as f32))
                                .w(px(((width_ratio * 380.0) as f32).max(4.0)))
                                .rounded_sm()
                                .bg(accent),
                        ),
                )
                .child(
                    div()
                        .w(px(70.0))
                        .text_right()
                        .flex_shrink_0()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(format_duration_us(stage.duration_us)),
                )
                .child(
                    div()
                        .w(px(46.0))
                        .text_right()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child(format!("{:.1}%", stage.percentage * 100.0)),
                );

            timeline_bars = timeline_bars.child(bar_row);
        }

        v_flex()
            .w_full()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .rounded_md()
            .overflow_hidden()
            .child(time_axis)
            .child(timeline_bars)
    }

    fn render_table(&self, report: &QueryProfileReport, lang: AppLanguage) -> impl IntoElement {
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
            .child(div().w(px(32.0)).child("#"))
            .child(div().w(px(220.0)).child(t("profile.col_stage", lang)))
            .child(div().w(px(110.0)).child(t("profile.col_category", lang)))
            .child(div().w(px(90.0)).child(t("profile.col_offset", lang)))
            .child(div().w(px(90.0)).child(t("profile.col_duration", lang)))
            .child(div().w(px(80.0)).child(t("profile.col_pct", lang)))
            .child(div().flex_1().child(t("profile.col_details", lang)));

        let mut rows = v_flex()
            .w_full()
            .border_l_1()
            .border_r_1()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .rounded_b_md()
            .overflow_hidden();

        for (idx, stage) in report.stages.iter().enumerate() {
            let row_bg = if idx % 2 == 1 {
                ThemeColors::BG_SURFACE
            } else {
                ThemeColors::BG_APP
            };

            let accent = Self::stage_accent_color(stage.category, stage.percentage);

            let row = h_flex()
                .w_full()
                .px_3()
                .py_1p5()
                .bg(row_bg)
                .items_center()
                .border_t_1()
                .border_color(ThemeColors::BORDER)
                .child(
                    div()
                        .w(px(32.0))
                        .text_size(px(11.0))
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child((idx + 1).to_string()),
                )
                .child(
                    h_flex()
                        .w(px(220.0))
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .w(px(6.0))
                                .h(px(6.0))
                                .rounded_full()
                                .bg(accent),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(stage.name.clone()),
                        ),
                )
                .child(
                    div()
                        .w(px(110.0))
                        .text_size(px(11.0))
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(stage.category.name().to_string()),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .text_size(px(11.0))
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(format!("{} µs", stage.start_time_us)),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(format!("{:.3} ms", stage.duration_ms())),
                )
                .child(
                    div()
                        .w(px(80.0))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(accent)
                        .child(format!("{:.1}%", stage.percentage * 100.0)),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .text_color(ThemeColors::TEXT_MUTED)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(stage.details.as_deref().unwrap_or("-").to_string()),
                );

            rows = rows.child(row);
        }

        v_flex().w_full().child(header).child(rows)
    }
}

impl RenderOnce for ProfilePanel {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        let lang = self.language;

        if self.is_profiling {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .child(Spinner::new())
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(t("profile.profiling_hint", lang)),
                )
                .into_any_element();
        }

        if let Some(ref err) = self.error {
            return v_flex()
                .size_full()
                .p_4()
                .child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(rgba(0xdc354515))
                        .border_1()
                        .border_color(ThemeColors::ERROR)
                        .text_xs()
                        .text_color(ThemeColors::ERROR)
                        .child(err.clone()),
                )
                .into_any_element();
        }

        let Some(report) = self.report.as_ref() else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Gauge)
                        .size(px(36.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(t("profile.no_profile_hint", lang)),
                )
                .into_any_element();
        };

        // KPI Row
        let kpi_row = h_flex()
            .w_full()
            .gap_2()
            .child(self.render_kpi_card(
                t("profile.kpi_total_duration", lang),
                format!("{:.2} ms", report.metrics.total_duration_ms()),
                ThemeColors::PRIMARY_LIGHT,
            ))
            .child(self.render_kpi_card(
                t("profile.kpi_planning_time", lang),
                report
                    .metrics
                    .planning_time_ms()
                    .map(|ms| format!("{ms:.2} ms"))
                    .unwrap_or_else(|| "-".to_string()),
                ThemeColors::WARNING,
            ))
            .child(self.render_kpi_card(
                t("profile.kpi_execution_time", lang),
                report
                    .metrics
                    .execution_time_ms()
                    .map(|ms| format!("{ms:.2} ms"))
                    .unwrap_or_else(|| "-".to_string()),
                ThemeColors::SUCCESS,
            ))
            .child(self.render_kpi_card(
                t("profile.kpi_buffer_hits", lang),
                report
                    .metrics
                    .buffer_hit_ratio()
                    .map(|r| format!("{:.1}%", r * 100.0))
                    .unwrap_or_else(|| "100.0%".to_string()),
                ThemeColors::PRIMARY_BORDER,
            ))
            .child(self.render_kpi_card(
                t("profile.kpi_bottleneck", lang),
                report.bottleneck_stage.clone().unwrap_or_else(|| "-".to_string()),
                ThemeColors::ERROR,
            ));

        // Toolbar
        let on_copy_action = self.on_copy.clone();
        let copy_btn = Button::new("copy_profile_btn")
            .outline()
            .small()
            .icon(if self.copied {
                IconName::Check
            } else {
                IconName::Copy
            })
            .label(if self.copied {
                t("profile.copied", lang)
            } else {
                t("profile.btn_copy_profile", lang)
            })
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_copy_action {
                    h(window, cx);
                }
            });

        let toolbar = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Gauge)
                            .size(px(14.0))
                            .text_color(ThemeColors::PRIMARY_LIGHT),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("profile.waterfall_title", lang)),
                    ),
            )
            .child(copy_btn);

        let waterfall = self.render_waterfall(report);
        let table = self.render_table(report, lang);

        v_flex()
            .id("profile_panel_scroll")
            .size_full()
            .min_h_0()
            .p_3()
            .gap_3()
            .overflow_y_scroll()
            .child(kpi_row)
            .child(toolbar)
            .child(waterfall)
            .child(table)
            .into_any_element()
    }
}
