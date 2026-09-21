//! Database connection error modal dialog.
//!
//! Provides clear diagnostic error display, contextual connection target info,
//! and actionable remediation buttons (Retry, Edit Connection, Copy Error).
//! Designed with strict overflow protection to prevent error banners from exceeding borders.

use crate::db::types::DatabaseType;
use crate::settings::AppLanguage;
use crate::ui::components::connection_dialog::database_icon;
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
    StatefulInteractiveElement as _, Styled, Window, div, px, rgba,
};
use std::rc::Rc;

/// State describing a failed database connection attempt
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionErrorInfo {
    pub connection_id: String,
    pub connection_name: String,
    pub database_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub error_message: String,
}

/// Break unbroken long tokens (e.g. unbroken error strings, URLs, long socket paths)
/// so they wrap cleanly without ever overflowing container boundaries.
pub fn wrap_error_text(input: &str, max_unbroken_len: usize) -> String {
    let mut result = String::with_capacity(input.len() + 32);

    for (line_idx, line) in input.lines().enumerate() {
        if line_idx > 0 {
            result.push('\n');
        }

        let mut current_unbroken_len = 0;
        for ch in line.chars() {
            if ch.is_whitespace() {
                result.push(ch);
                current_unbroken_len = 0;
            } else {
                if current_unbroken_len >= max_unbroken_len {
                    result.push('\n');
                    current_unbroken_len = 0;
                }
                result.push(ch);
                current_unbroken_len += 1;
            }
        }
    }

    result
}

#[derive(IntoElement)]
pub struct ConnectionErrorDialog {
    info: ConnectionErrorInfo,
    language: AppLanguage,
    copied: bool,
    is_retrying: bool,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_retry: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
    on_edit: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
    on_copy: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
}

impl ConnectionErrorDialog {
    pub fn new(info: ConnectionErrorInfo) -> Self {
        Self {
            info,
            language: AppLanguage::En,
            copied: false,
            is_retrying: false,
            on_close: None,
            on_retry: None,
            on_edit: None,
            on_copy: None,
        }
    }

    pub fn language(mut self, language: AppLanguage) -> Self {
        self.language = language;
        self
    }

    pub fn copied(mut self, copied: bool) -> Self {
        self.copied = copied;
        self
    }

    pub fn retrying(mut self, retrying: bool) -> Self {
        self.is_retrying = retrying;
        self
    }

    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    pub fn on_retry<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    pub fn on_edit<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_edit = Some(Rc::new(handler));
        self
    }

    pub fn on_copy<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_copy = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ConnectionErrorDialog {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let lang = self.language;
        let conn_id = self.info.connection_id.clone();
        let conn_id_for_edit = self.info.connection_id.clone();
        let err_raw = self.info.error_message.clone();
        let safe_err_display = wrap_error_text(&err_raw, 64);

        let title = t("conn_error.title", lang);
        let subtitle = t("conn_error.subtitle", lang);
        let details_label = t("conn_error.details", lang);
        let retry_label = t("conn_error.retry", lang);
        let edit_label = t("conn_error.edit", lang);
        let close_label = t("conn_error.close", lang);
        let copy_label = if self.copied {
            t("conn_error.copied", lang)
        } else {
            t("conn_error.copy", lang)
        };

        let on_close = self.on_close.clone();
        let on_retry = self.on_retry.clone();
        let on_edit = self.on_edit.clone();
        let on_copy = self.on_copy.clone();

        // Target connection metadata badge
        let target_summary = if self.info.database_type.is_file_based() {
            if self.info.database.is_empty() {
                ":memory:".to_string()
            } else {
                self.info.database.clone()
            }
        } else {
            let mut s = format!("{}:{}", self.info.host, self.info.port);
            if !self.info.database.is_empty() {
                s.push_str(&format!(" / {}", self.info.database));
            }
            s
        };

        let target_badge = h_flex()
            .w_full()
            .min_w_0()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div().flex_shrink_0().child(
                            Icon::new(database_icon(self.info.database_type))
                                .size(px(16.0))
                                .text_color(ThemeColors::PRIMARY_BORDER),
                        ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(self.info.connection_name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(ThemeColors::BG_APP)
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(self.info.database_type.display_name()),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(px(220.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(target_summary),
            );

        // Bounded Error Banner: Guaranteed to never overflow container borders
        let copy_handler_inline = on_copy.clone();
        let err_for_inline_copy = err_raw.clone();
        let error_banner = v_flex()
            .w_full()
            .min_w_0()
            .rounded_md()
            .bg(rgba(0xEF444415))
            .border_1()
            .border_color(ThemeColors::ERROR)
            .overflow_hidden()
            .child(
                // Error bar header
                h_flex()
                    .w_full()
                    .min_w_0()
                    .justify_between()
                    .items_center()
                    .px_3()
                    .py_2()
                    .bg(rgba(0xEF444422))
                    .border_b_1()
                    .border_color(rgba(0xEF444444))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .min_w_0()
                            .child(
                                Icon::new(IconName::TriangleAlert)
                                    .size(px(14.0))
                                    .text_color(ThemeColors::ERROR),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::ERROR)
                                    .child(details_label),
                            ),
                    )
                    .child({
                        let mut btn = Button::new("copy_error_inline_btn")
                            .ghost()
                            .xsmall()
                            .icon(if self.copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .label(copy_label);
                        if let Some(ref handler) = copy_handler_inline {
                            let handler = handler.clone();
                            let text = err_for_inline_copy;
                            btn = btn.on_click(move |_, window, cx| {
                                handler(&text, window, cx);
                            });
                        }
                        btn
                    }),
            )
            .child(
                // Scrollable, wrapped error text container
                v_flex()
                    .id("conn_error_message_box")
                    .w_full()
                    .min_w_0()
                    .max_h(px(160.0))
                    .p_3()
                    .overflow_y_scroll()
                    .whitespace_normal()
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(ThemeColors::ERROR)
                    .child(safe_err_display),
            );

        // Header icon and title
        let header = h_flex()
            .w_full()
            .min_w_0()
            .justify_between()
            .items_start()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .flex_shrink_0()
                            .size(px(36.0))
                            .rounded_full()
                            .bg(rgba(0xEF444420))
                            .border_1()
                            .border_color(rgba(0xEF444450))
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::TriangleAlert)
                                    .size(px(20.0))
                                    .text_color(ThemeColors::ERROR),
                            ),
                    )
                    .child(
                        v_flex()
                            .min_w_0()
                            .flex_1()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .whitespace_normal()
                                    .child(subtitle),
                            ),
                    ),
            )
            .child({
                let mut close_x_btn = Button::new("close_error_dialog_x")
                    .ghost()
                    .xsmall()
                    .icon(IconName::X);
                if let Some(ref handler) = on_close {
                    let handler = handler.clone();
                    close_x_btn = close_x_btn.on_click(move |_, window, cx| {
                        handler(window, cx);
                    });
                }
                close_x_btn
            });

        // Footer action bar
        let footer = h_flex()
            .w_full()
            .min_w_0()
            .justify_between()
            .items_center()
            .pt_2()
            .child({
                let mut copy_btn = Button::new("conn_error_copy_btn")
                    .outline()
                    .small()
                    .icon(if self.copied {
                        IconName::Check
                    } else {
                        IconName::Copy
                    })
                    .label(copy_label);
                if let Some(ref handler) = on_copy {
                    let handler = handler.clone();
                    let text = err_raw;
                    copy_btn = copy_btn.on_click(move |_, window, cx| {
                        handler(&text, window, cx);
                    });
                }
                copy_btn
            })
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child({
                        let mut edit_btn = Button::new("conn_error_edit_btn")
                            .outline()
                            .small()
                            .icon(IconName::Pencil)
                            .label(edit_label);
                        if let Some(ref handler) = on_edit {
                            let handler = handler.clone();
                            let cid = conn_id_for_edit;
                            edit_btn = edit_btn.on_click(move |_, window, cx| {
                                handler(&cid, window, cx);
                            });
                        }
                        edit_btn
                    })
                    .child({
                        let mut retry_btn = Button::new("conn_error_retry_btn")
                            .primary()
                            .small()
                            .icon(IconName::RotateCcw)
                            .label(retry_label);
                        if let Some(ref handler) = on_retry {
                            let handler = handler.clone();
                            let cid = conn_id;
                            retry_btn = retry_btn.on_click(move |_, window, cx| {
                                handler(&cid, window, cx);
                            });
                        }
                        retry_btn
                    })
                    .child({
                        let mut dismiss_btn =
                            Button::new("conn_error_close_btn").ghost().small().label(close_label);
                        if let Some(ref handler) = on_close {
                            let handler = handler.clone();
                            dismiss_btn = dismiss_btn.on_click(move |_, window, cx| {
                                handler(window, cx);
                            });
                        }
                        dismiss_btn
                    }),
            );

        // Modal card container
        let modal = v_flex()
            .w(px(520.0))
            .max_w(px(560.0))
            .rounded_xl()
            .border_1()
            .border_color(ThemeColors::BORDER_PROMINENT)
            .bg(ThemeColors::BG_APP)
            .shadow_lg()
            .overflow_hidden()
            .p_5()
            .gap_4()
            .child(header)
            .child(target_badge)
            .child(error_banner)
            .child(footer);

        // Modal Backdrop
        div()
            .id("connection_error_modal_backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000B0))
            .flex()
            .items_center()
            .justify_center()
            .child(modal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_error_text_normal_sentences() {
        let text = "Connection refused by host 127.0.0.1:5432";
        let wrapped = wrap_error_text(text, 64);
        assert_eq!(wrapped, text);
    }

    #[test]
    fn test_wrap_error_text_breaks_unbroken_long_token() {
        let long_token = "A".repeat(150);
        let wrapped = wrap_error_text(&long_token, 64);
        let lines: Vec<&str> = wrapped.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].len(), 64);
        assert_eq!(lines[1].len(), 64);
        assert_eq!(lines[2].len(), 22);
    }

    #[test]
    fn test_wrap_error_text_preserves_existing_newlines() {
        let multiline = "Line 1\nLine 2\nLine 3";
        let wrapped = wrap_error_text(multiline, 64);
        assert_eq!(wrapped, multiline);
    }

    #[test]
    fn test_connection_error_info_construction() {
        let info = ConnectionErrorInfo {
            connection_id: "conn-1".to_string(),
            connection_name: "Local Postgres".to_string(),
            database_type: DatabaseType::Postgres,
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            error_message: "password authentication failed for user 'postgres'".to_string(),
        };

        assert_eq!(info.connection_id, "conn-1");
        assert_eq!(info.port, 5432);
        assert!(!info.error_message.is_empty());
    }
}
