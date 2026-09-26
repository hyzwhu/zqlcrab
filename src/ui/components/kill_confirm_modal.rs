//! Modal dialog for safely confirming Cancel Query or Terminate Session actions.

use crate::db::session_monitor::{KillAction, SessionInfo, SessionLatencySeverity};
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px, rgba,
};
use std::rc::Rc;

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct KillConfirmModal {
    session: SessionInfo,
    action: KillAction,
    execute_sql: String,
    language: AppLanguage,
    on_confirm: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_cancel: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl KillConfirmModal {
    pub fn new(
        session: SessionInfo,
        action: KillAction,
        execute_sql: String,
        language: AppLanguage,
    ) -> Self {
        Self {
            session,
            action,
            execute_sql,
            language,
            on_confirm: None,
            on_cancel: None,
        }
    }

    pub fn on_confirm<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_confirm = Some(Rc::new(handler));
        self
    }

    pub fn on_cancel<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_cancel = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for KillConfirmModal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let cancel_handler = self.on_cancel.clone();
        let confirm_handler = self.on_confirm.clone();

        let title = match self.action {
            KillAction::CancelQuery => t("sessions.cancel_modal_title", self.language),
            KillAction::TerminateSession => t("sessions.terminate_modal_title", self.language),
        };

        let is_terminate = self.action == KillAction::TerminateSession;
        let warning_color = if is_terminate {
            ThemeColors::ERROR
        } else {
            ThemeColors::WARNING
        };

        let severity_badge = match self.session.latency_severity() {
            SessionLatencySeverity::Critical => div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0xef444420))
                .text_color(rgba(0xf87171ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child(format!("{}s (Critical)", self.session.duration_secs)),
            SessionLatencySeverity::Warning => div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0xf59e0b20))
                .text_color(rgba(0xfbbf24ff))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{}s (Warning)", self.session.duration_secs)),
            SessionLatencySeverity::Notice => div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0x3b82f620))
                .text_color(rgba(0x60a5faff))
                .text_xs()
                .child(format!("{}s", self.session.duration_secs)),
            SessionLatencySeverity::Normal => div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                .text_color(ThemeColors::TEXT_MUTED)
                .text_xs()
                .child(format!("{}s", self.session.duration_secs)),
        };

        let query_text = if self.session.query.is_empty() {
            "<no active query>".to_string()
        } else {
            self.session.query.clone()
        };

        let dialog = v_flex()
            .w(px(520.0))
            .rounded_lg()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .shadow_lg()
            .overflow_hidden()
            // Header
            .child(
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
                            .gap_2()
                            .child(
                                Icon::new(IconName::TriangleAlert)
                                    .size(px(18.0))
                                    .text_color(warning_color),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_base()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(title),
                            ),
                    )
                    .child(
                        Button::new("kill_modal_close_btn")
                            .small()
                            .ghost()
                            .icon(IconName::Close)
                            .when_some(cancel_handler.clone(), |btn, h| {
                                btn.on_click(move |_, window, cx| h(window, cx))
                            }),
                    ),
            )
            // Body Details
            .child(
                v_flex()
                    .w_full()
                    .p_5()
                    .gap_3()
                    .child(div().text_xs().text_color(ThemeColors::TEXT_MUTED).child(
                        if is_terminate {
                            t("sessions.terminate_modal_desc", self.language)
                        } else {
                            t("sessions.cancel_modal_desc", self.language)
                        },
                    ))
                    .child(
                        v_flex()
                            .w_full()
                            .p_3()
                            .gap_2()
                            .rounded_md()
                            .bg(ThemeColors::BG_APP)
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .child(
                                        div().text_xs().text_color(ThemeColors::TEXT_MUTED).child(
                                            format!("Session PID / ID: {}", self.session.id),
                                        ),
                                    )
                                    .child(severity_badge),
                            )
                            .child(div().text_xs().text_color(ThemeColors::TEXT_PRIMARY).child(
                                format!(
                                    "User: {}  |  Database: {}  |  Client: {}",
                                    if self.session.username.is_empty() {
                                        "unknown"
                                    } else {
                                        &self.session.username
                                    },
                                    if self.session.database.is_empty() {
                                        "default"
                                    } else {
                                        &self.session.database
                                    },
                                    if self.session.client_addr.is_empty() {
                                        "local"
                                    } else {
                                        &self.session.client_addr
                                    }
                                ),
                            )),
                    )
                    // Current Query Preview
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("sessions.current_sql", self.language)),
                            )
                            .child(
                                div()
                                    .id("kill_query_scroll")
                                    .w_full()
                                    .max_h(px(90.0))
                                    .overflow_y_scroll()
                                    .p_2()
                                    .rounded_sm()
                                    .bg(ThemeColors::BG_APP)
                                    .border_1()
                                    .border_color(ThemeColors::BORDER)
                                    .text_xs()
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(query_text),
                            ),
                    )
                    // SQL Command To Execute
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("sessions.command_to_run", self.language)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .p_2()
                                    .rounded_sm()
                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                    .border_1()
                                    .border_color(warning_color)
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_color(warning_color)
                                    .child(self.execute_sql.clone()),
                            ),
                    ),
            )
            // Footer
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_3()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_APP)
                    .child(
                        Button::new("kill_modal_cancel")
                            .ghost()
                            .label(t("common.cancel", self.language))
                            .when_some(cancel_handler, |btn, h| {
                                btn.on_click(move |_, window, cx| h(window, cx))
                            }),
                    )
                    .child(
                        Button::new("kill_modal_confirm")
                            .danger()
                            .icon(IconName::TriangleAlert)
                            .label(if is_terminate {
                                t("sessions.btn_confirm_terminate", self.language)
                            } else {
                                t("sessions.btn_confirm_cancel", self.language)
                            })
                            .when_some(confirm_handler, |btn, h| {
                                btn.on_click(move |_, window, cx| h(window, cx))
                            }),
                    ),
            );

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

    #[test]
    fn test_kill_confirm_modal_creation() {
        let session = SessionInfo {
            id: "42".to_string(),
            username: "tester".to_string(),
            database: "testdb".to_string(),
            client_addr: "127.0.0.1".to_string(),
            state: "active".to_string(),
            duration_secs: 25,
            wait_event: "Lock".to_string(),
            query: "SELECT * FROM t".to_string(),
            is_current: false,
        };

        assert_eq!(session.latency_severity(), SessionLatencySeverity::Warning);
        let modal = KillConfirmModal::new(
            session,
            KillAction::CancelQuery,
            "SELECT pg_cancel_backend(42);".to_string(),
            AppLanguage::En,
        );
        assert_eq!(modal.action, KillAction::CancelQuery);
    }
}
