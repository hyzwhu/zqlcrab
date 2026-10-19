//! High-visibility modal dialog for intercepting and confirming destructive SQL operations in Production environments.

use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::*, px, rgba,
};
use std::rc::Rc;

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct ProductionSafeguardModal {
    connection_name: String,
    database_name: String,
    risk_summary: &'static str,
    sql: String,
    language: AppLanguage,
    on_confirm: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_cancel: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl ProductionSafeguardModal {
    pub fn new(
        connection_name: impl Into<String>,
        database_name: impl Into<String>,
        risk_summary: &'static str,
        sql: impl Into<String>,
        language: AppLanguage,
    ) -> Self {
        Self {
            connection_name: connection_name.into(),
            database_name: database_name.into(),
            risk_summary,
            sql: sql.into(),
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

impl RenderOnce for ProductionSafeguardModal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let cancel_handler = self.on_cancel.clone();
        let confirm_handler = self.on_confirm.clone();

        let sql_display = if self.sql.trim().is_empty() {
            "-- [Empty Statement]".to_string()
        } else {
            self.sql.trim().to_string()
        };

        let modal = v_flex()
            .occlude()
            .w(px(560.0))
            .rounded_xl()
            .border_2()
            .border_color(ThemeColors::ERROR)
            .bg(ThemeColors::BG_SURFACE)
            .shadow_lg()
            .overflow_hidden()
            // High-Alert Header Banner
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_3p5()
                    .items_center()
                    .justify_between()
                    .bg(ThemeColors::ERROR)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                Icon::new(IconName::ShieldAlert)
                                    .size(px(20.0))
                                    .text_color(ThemeColors::TEXT_PRIMARY),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_base()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("safeguard.modal_title", self.language)),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(rgba(0x00000040))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("PROD"),
                    ),
            )
            // Body Content
            .child(
                v_flex()
                    .w_full()
                    .p_5()
                    .gap_3p5()
                    // Warning Description Box
                    .child(
                        v_flex()
                            .w_full()
                            .p_3()
                            .gap_1p5()
                            .rounded_lg()
                            .bg(ThemeColors::BG_APP)
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(ThemeColors::ERROR)
                                            .child(t("safeguard.target_database", self.language)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(format!(
                                                "{} ({})",
                                                self.connection_name,
                                                if self.database_name.is_empty() {
                                                    "default"
                                                } else {
                                                    &self.database_name
                                                }
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("safeguard.warning_desc", self.language)),
                            ),
                    )
                    // Detected Risk Alert
                    .child(
                        v_flex()
                            .w_full()
                            .p_3()
                            .gap_1()
                            .rounded_lg()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .border_1()
                            .border_color(ThemeColors::ERROR)
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::ERROR)
                                    .child(t("safeguard.detected_risk", self.language)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(self.risk_summary),
                            ),
                    )
                    // SQL Preview Box
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("safeguard.statement_to_run", self.language)),
                            )
                            .child(
                                div()
                                    .id("safeguard_sql_scroll")
                                    .w_full()
                                    .max_h(px(140.0))
                                    .overflow_y_scroll()
                                    .p_3()
                                    .rounded_lg()
                                    .bg(ThemeColors::BG_APP)
                                    .border_1()
                                    .border_color(ThemeColors::BORDER)
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(sql_display),
                            ),
                    ),
            )
            // Footer
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_3p5()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(t("safeguard.esc_to_cancel", self.language)),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("safeguard_cancel_btn")
                                    .ghost()
                                    .label(t("common.cancel", self.language))
                                    .when_some(cancel_handler, |btn, h| {
                                        btn.on_click(move |_, window, cx| h(window, cx))
                                    }),
                            )
                            .child(
                                Button::new("safeguard_confirm_btn")
                                    .danger()
                                    .icon(IconName::ShieldAlert)
                                    .label(t("safeguard.btn_confirm_execute", self.language))
                                    .when_some(confirm_handler, |btn, h| {
                                        btn.on_click(move |_, window, cx| h(window, cx))
                                    }),
                            ),
                    ),
            );

        div()
            .id("production_safeguard_modal_backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000C0))
            .justify_center()
            .items_center()
            .flex()
            .child(modal)
    }
}
