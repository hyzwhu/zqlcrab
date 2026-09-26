//! Multi-dialect active session and process list monitoring dashboard view.

use crate::db::session_monitor::{KillAction, SessionInfo, SessionLatencySeverity};
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
pub struct SessionsView {
    sessions: Vec<SessionInfo>,
    active_database: Option<String>,
    dialect: Option<DatabaseFamily>,
    search_input: Entity<InputState>,
    auto_refresh_secs: u32,
    is_refreshing: bool,
    language: AppLanguage,
    on_refresh: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_set_auto_refresh: Option<Rc<dyn Fn(u32, &mut Window, &mut App) + 'static>>,
    on_kill_action: Option<Rc<dyn Fn(SessionInfo, KillAction, &mut Window, &mut App) + 'static>>,
    on_copy_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_open_query_in_editor: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
}

impl SessionsView {
    pub fn new(
        sessions: Vec<SessionInfo>,
        active_database: Option<String>,
        dialect: Option<DatabaseFamily>,
        search_input: Entity<InputState>,
        auto_refresh_secs: u32,
        is_refreshing: bool,
        language: AppLanguage,
    ) -> Self {
        Self {
            sessions,
            active_database,
            dialect,
            search_input,
            auto_refresh_secs,
            is_refreshing,
            language,
            on_refresh: None,
            on_set_auto_refresh: None,
            on_kill_action: None,
            on_copy_query: None,
            on_open_query_in_editor: None,
        }
    }

    pub fn on_refresh<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh = Some(Rc::new(handler));
        self
    }

    pub fn on_set_auto_refresh<F>(mut self, handler: F) -> Self
    where
        F: Fn(u32, &mut Window, &mut App) + 'static,
    {
        self.on_set_auto_refresh = Some(Rc::new(handler));
        self
    }

    pub fn on_kill_action<F>(mut self, handler: F) -> Self
    where
        F: Fn(SessionInfo, KillAction, &mut Window, &mut App) + 'static,
    {
        self.on_kill_action = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_query<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_query = Some(Rc::new(handler));
        self
    }

    pub fn on_open_query_in_editor<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_open_query_in_editor = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SessionsView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let query_filter = self.search_input.read(cx).value().to_lowercase();

        let filtered_sessions: Vec<&SessionInfo> = self
            .sessions
            .iter()
            .filter(|s| {
                if query_filter.is_empty() {
                    return true;
                }
                s.id.to_lowercase().contains(&query_filter)
                    || s.username.to_lowercase().contains(&query_filter)
                    || s.database.to_lowercase().contains(&query_filter)
                    || s.client_addr.to_lowercase().contains(&query_filter)
                    || s.state.to_lowercase().contains(&query_filter)
                    || s.query.to_lowercase().contains(&query_filter)
                    || s.wait_event.to_lowercase().contains(&query_filter)
            })
            .collect();

        let total_count = self.sessions.len();
        let active_count = self
            .sessions
            .iter()
            .filter(|s| {
                s.state.eq_ignore_ascii_case("active")
                    || s.state.eq_ignore_ascii_case("query")
                    || (!s.query.is_empty() && !s.state.contains("idle"))
            })
            .count();
        let slow_count = self
            .sessions
            .iter()
            .filter(|s| s.duration_secs >= 15)
            .count();

        // 1. Header Toolbar
        let refresh_btn = {
            let handler = self.on_refresh.clone();
            let is_refreshing = self.is_refreshing;
            Button::new("sessions_refresh_btn")
                .small()
                .ghost()
                .icon(IconName::RotateCw)
                .label(if is_refreshing {
                    t("sessions.refreshing", self.language)
                } else {
                    t("sessions.refresh", self.language)
                })
                .when_some(handler, |btn, h| {
                    btn.on_click(move |_, window, cx| h(window, cx))
                })
        };

        let auto_refresh_intervals = [(0, "Off"), (5, "5s"), (10, "10s"), (30, "30s")];
        let mut auto_refresh_pills = h_flex().items_center().gap_1();
        for (secs, label) in auto_refresh_intervals {
            let is_selected = self.auto_refresh_secs == secs;
            let handler = self.on_set_auto_refresh.clone();
            let pill = div()
                .id(ElementId::Name(format!("auto_ref_{}", label).into()))
                .px_2()
                .py_0p5()
                .rounded_full()
                .text_xs()
                .cursor_pointer()
                .bg(if is_selected {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BG_SURFACE_ACTIVE
                })
                .text_color(if is_selected {
                    ThemeColors::PRIMARY
                } else {
                    ThemeColors::TEXT_MUTED
                })
                .hover(|s| {
                    s.bg(ThemeColors::PRIMARY_BORDER)
                        .text_color(ThemeColors::PRIMARY)
                })
                .child(label)
                .when_some(handler, |el, h| {
                    el.on_click(move |_, window, cx| h(secs, window, cx))
                });
            auto_refresh_pills = auto_refresh_pills.child(pill);
        }

        let header = h_flex()
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
                    .gap_3()
                    .child(
                        Icon::new(IconName::Activity)
                            .size(px(20.0))
                            .text_color(ThemeColors::PRIMARY),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_base()
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("sessions.title", self.language)),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!("Total: {}", total_count)),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgba(0x3b82f620))
                                    .text_xs()
                                    .text_color(rgba(0x60a5faff))
                                    .child(format!("Active: {}", active_count)),
                            )
                            .when(slow_count > 0, |el| {
                                el.child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(rgba(0xef444420))
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgba(0xf87171ff))
                                        .child(format!("Slow: {}", slow_count)),
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("sessions.auto_refresh", self.language)),
                            )
                            .child(auto_refresh_pills),
                    )
                    .child(refresh_btn),
            );

        // 2. Search & Dialect Status Subheader
        let dialect_label = self
            .dialect
            .map(|d| match d {
                DatabaseFamily::Postgres => "PostgreSQL (pg_stat_activity)",
                DatabaseFamily::MySql => "MySQL (information_schema.PROCESSLIST)",
                DatabaseFamily::Sqlite => "SQLite (PRAGMA database_list)",
            })
            .unwrap_or("No Active Connection");

        let search_bar = Input::new(&self.search_input)
            .cleanable(true)
            .prefix(Icon::new(IconName::Search).size(px(14.0)));

        let subheader = h_flex()
            .w_full()
            .px_5()
            .py_2p5()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            .child(div().w(px(320.0)).child(search_bar))
            .child(
                h_flex().items_center().gap_2().child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(format!(
                            "{} • {}",
                            self.active_database.as_deref().unwrap_or("default"),
                            dialect_label
                        )),
                ),
            );

        // 3. Process Table / Cards Content
        let content = if filtered_sessions.is_empty() {
            div()
                .id("sessions_empty_state")
                .flex_1()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Activity)
                        .size(px(36.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(div().text_sm().text_color(ThemeColors::TEXT_MUTED).child(
                    if total_count == 0 {
                        t("sessions.no_sessions_found", self.language)
                    } else {
                        t("sessions.no_matching_filter", self.language)
                    },
                ))
        } else {
            let is_sqlite = self.dialect == Some(DatabaseFamily::Sqlite);
            let mut list = v_flex().gap_2().p_4();

            for session in filtered_sessions {
                let s_clone_cancel = session.clone();
                let s_clone_term = session.clone();
                let query_text = session.query.clone();
                let query_for_copy = session.query.clone();
                let query_for_editor = session.query.clone();

                let kill_handler_cancel = self.on_kill_action.clone();
                let kill_handler_term = self.on_kill_action.clone();
                let copy_handler = self.on_copy_query.clone();
                let editor_handler = self.on_open_query_in_editor.clone();

                let severity_badge = match session.latency_severity() {
                    SessionLatencySeverity::Critical => div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0xef444420))
                        .text_color(rgba(0xf87171ff))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .child(format!("{}s", session.duration_secs)),
                    SessionLatencySeverity::Warning => div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0xf59e0b20))
                        .text_color(rgba(0xfbbf24ff))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{}s", session.duration_secs)),
                    SessionLatencySeverity::Notice => div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0x3b82f620))
                        .text_color(rgba(0x60a5faff))
                        .text_xs()
                        .child(format!("{}s", session.duration_secs)),
                    SessionLatencySeverity::Normal => div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::BG_SURFACE_ACTIVE)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .text_xs()
                        .child(format!("{}s", session.duration_secs)),
                };

                let state_color = if session.state.eq_ignore_ascii_case("active")
                    || session.state.eq_ignore_ascii_case("query")
                {
                    rgba(0x10b981ff) // green
                } else if session.state.contains("idle") {
                    rgba(0x94a3b8ff) // muted grey
                } else {
                    rgba(0xf59e0bff) // amber
                };

                let card = v_flex()
                    .id(ElementId::Name(
                        format!("session_card_{}", session.id).into(),
                    ))
                    .w_full()
                    .p_3()
                    .gap_2()
                    .rounded_md()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .hover(|s| s.border_color(ThemeColors::BORDER_PROMINENT))
                    .child(
                        // Row 1: PID, User, Database, Host, Status, Latency Badge, Actions
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_xs()
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text_color(ThemeColors::PRIMARY)
                                            .child(format!("PID {}", session.id)),
                                    )
                                    .when(session.is_current, |el| {
                                        el.child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgba(0x8b5cf620))
                                                .text_color(rgba(0xa78bfafe))
                                                .text_xs()
                                                .child("Current"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                            .text_color(state_color)
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(session.state.clone()),
                                    )
                                    .child(severity_badge)
                                    .child(
                                        div().text_xs().text_color(ThemeColors::TEXT_MUTED).child(
                                            format!(
                                                "{} @ {} ({})",
                                                if session.username.is_empty() {
                                                    "-"
                                                } else {
                                                    &session.username
                                                },
                                                if session.database.is_empty() {
                                                    "-"
                                                } else {
                                                    &session.database
                                                },
                                                if session.client_addr.is_empty() {
                                                    "local"
                                                } else {
                                                    &session.client_addr
                                                }
                                            ),
                                        ),
                                    )
                                    .when(!session.wait_event.is_empty(), |el| {
                                        el.child(
                                            div()
                                                .text_xs()
                                                .text_color(rgba(0xfbbf24ff))
                                                .child(format!("Wait: {}", session.wait_event)),
                                        )
                                    }),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1()
                                    // Copy SQL Button
                                    .when(!session.query.is_empty(), |row| {
                                        row.child(
                                            Button::new(ElementId::Name(
                                                format!("copy_q_{}", session.id).into(),
                                            ))
                                            .small()
                                            .ghost()
                                            .icon(IconName::Copy)
                                            .tooltip(t("sessions.tooltip_copy_sql", self.language))
                                            .when_some(copy_handler, |btn, h| {
                                                btn.on_click(move |_, window, cx| {
                                                    h(query_for_copy.clone(), window, cx)
                                                })
                                            }),
                                        )
                                        .child(
                                            Button::new(ElementId::Name(
                                                format!("edit_q_{}", session.id).into(),
                                            ))
                                            .small()
                                            .ghost()
                                            .icon(IconName::SquareArrowOutUpRight)
                                            .tooltip(t(
                                                "sessions.tooltip_open_editor",
                                                self.language,
                                            ))
                                            .when_some(editor_handler, |btn, h| {
                                                btn.on_click(move |_, window, cx| {
                                                    h(query_for_editor.clone(), window, cx)
                                                })
                                            }),
                                        )
                                    })
                                    // Cancel Query Button (disabled if SQLite or is_current or no query)
                                    .when(!is_sqlite && !session.query.is_empty(), |row| {
                                        row.child(
                                            Button::new(ElementId::Name(
                                                format!("cancel_q_{}", session.id).into(),
                                            ))
                                            .small()
                                            .ghost()
                                            .icon(IconName::Close)
                                            .label(t("sessions.btn_cancel_query", self.language))
                                            .when_some(kill_handler_cancel, |btn, h| {
                                                let s = s_clone_cancel.clone();
                                                btn.on_click(move |_, window, cx| {
                                                    h(
                                                        s.clone(),
                                                        KillAction::CancelQuery,
                                                        window,
                                                        cx,
                                                    )
                                                })
                                            }),
                                        )
                                    })
                                    // Terminate Session Button (disabled if SQLite or is_current)
                                    .when(!is_sqlite && !session.is_current, |row| {
                                        row.child(
                                            Button::new(ElementId::Name(
                                                format!("term_s_{}", session.id).into(),
                                            ))
                                            .small()
                                            .danger()
                                            .icon(IconName::TriangleAlert)
                                            .label(t(
                                                "sessions.btn_terminate_session",
                                                self.language,
                                            ))
                                            .when_some(kill_handler_term, |btn, h| {
                                                let s = s_clone_term.clone();
                                                btn.on_click(move |_, window, cx| {
                                                    h(
                                                        s.clone(),
                                                        KillAction::TerminateSession,
                                                        window,
                                                        cx,
                                                    )
                                                })
                                            }),
                                        )
                                    }),
                            ),
                    )
                    // Row 2: SQL Statement Preview Box
                    .when(!query_text.is_empty(), |el| {
                        el.child(
                            div()
                                .w_full()
                                .p_2()
                                .rounded_sm()
                                .bg(ThemeColors::BG_APP)
                                .border_1()
                                .border_color(ThemeColors::BORDER)
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text_xs()
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .max_h(px(40.0))
                                .overflow_hidden()
                                .child(query_text),
                        )
                    });

                list = list.child(card);
            }

            div()
                .id("sessions_cards_scroll")
                .flex_1()
                .overflow_y_scroll()
                .child(list)
        };

        v_flex()
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(header)
            .child(subheader)
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sessions_filtering() {
        let sessions = vec![
            SessionInfo {
                id: "101".to_string(),
                username: "alice".to_string(),
                database: "db1".to_string(),
                client_addr: "10.0.0.1".to_string(),
                state: "active".to_string(),
                duration_secs: 70,
                wait_event: "Lock:tuple".to_string(),
                query: "SELECT * FROM orders WHERE status = 'pending'".to_string(),
                is_current: false,
            },
            SessionInfo {
                id: "102".to_string(),
                username: "bob".to_string(),
                database: "db2".to_string(),
                client_addr: "10.0.0.2".to_string(),
                state: "idle".to_string(),
                duration_secs: 2,
                wait_event: "".to_string(),
                query: "".to_string(),
                is_current: true,
            },
        ];

        let critical = sessions
            .iter()
            .filter(|s| s.latency_severity() == SessionLatencySeverity::Critical)
            .count();
        assert_eq!(critical, 1);

        let active = sessions
            .iter()
            .filter(|s| s.state == "active" || (!s.query.is_empty() && !s.state.contains("idle")))
            .count();
        assert_eq!(active, 1);
    }
}
