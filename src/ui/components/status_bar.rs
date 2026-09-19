//! Status bar component displaying database session metrics and engine information.

use crate::db::types::ConnectionStatus;
use crate::ui::theme::ThemeColors;
use gpui_kit::base::h_flex;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::gpui::{
    App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::*, px,
};

#[derive(IntoElement)]
pub struct AppStatusBar {
    pub is_connected: bool,
    pub is_read_only: bool,
    pub profile_name: Option<SharedString>,
    pub db_type: Option<SharedString>,
    pub status: Option<ConnectionStatus>,
    pub row_count: Option<usize>,
    pub execution_time_ms: Option<u64>,
}

impl AppStatusBar {
    pub fn new() -> Self {
        Self {
            is_connected: false,
            is_read_only: false,
            profile_name: None,
            db_type: None,
            status: None,
            row_count: None,
            execution_time_ms: None,
        }
    }

    pub fn connected(mut self, connected: bool) -> Self {
        self.is_connected = connected;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.is_read_only = read_only;
        self
    }

    pub fn profile(mut self, name: impl Into<SharedString>) -> Self {
        self.profile_name = Some(name.into());
        self
    }

    pub fn engine(mut self, db_type: impl Into<SharedString>) -> Self {
        self.db_type = Some(db_type.into());
        self
    }

    pub fn status(mut self, status: Option<ConnectionStatus>) -> Self {
        self.status = status;
        self
    }

    pub fn query_stats(mut self, row_count: Option<usize>, exec_time: Option<u64>) -> Self {
        self.row_count = row_count;
        self.execution_time_ms = exec_time;
        self
    }
}

impl RenderOnce for AppStatusBar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (status_color, status_text) = if self.is_connected {
            (ThemeColors::SUCCESS, "Connected")
        } else {
            (ThemeColors::TEXT_FAINT, "Disconnected")
        };

        let left_part = h_flex()
            .items_center()
            .gap_2()
            .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
            .child(
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(status_text),
            )
            .when_some(self.profile_name, |this, name| {
                this.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(name),
                )
            })
            .when_some(self.db_type, |this, engine| {
                this.child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::BG_SURFACE_ACTIVE)
                        .text_xs()
                        .text_color(ThemeColors::PRIMARY_BORDER)
                        .child(engine),
                )
            })
            .when(self.is_read_only, |this| {
                this.child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::WARNING)
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("READ-ONLY"),
                )
            })
            .when_some(self.status, |this, stat| {
                this.when_some(stat.server_version, |sub, ver| {
                    sub.child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(ver),
                    )
                })
                .when_some(stat.ping_ms, |sub, ping| {
                    if ping == 0 {
                        sub
                    } else {
                        sub.child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!("{ping}ms")),
                        )
                    }
                })
            });

        let right_part = h_flex()
            .items_center()
            .gap_3()
            .when_some(self.row_count, |this, count| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(format!("{count} rows")),
                )
            })
            .when_some(self.execution_time_ms, |this, time| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::PRIMARY_BORDER)
                        .child(format!("{time} ms")),
                )
            })
            .child(
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_FAINT)
                    .child("zqlcrab v0.1.0"),
            );

        StatusBar::new().left(left_part).right(right_part)
    }
}
