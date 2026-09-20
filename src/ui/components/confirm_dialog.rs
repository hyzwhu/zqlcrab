//! Destructive action confirmation modal (Drop Table, Drop View, Truncate Table).

use crate::db::types::{DatabaseFamily, TableInfo};
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Styled,
    Window, div, px, rgba,
};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmActionKind {
    DropTable,
    DropView,
    TruncateTable,
}

impl ConfirmActionKind {
    pub fn is_drop(&self) -> bool {
        matches!(self, Self::DropTable | Self::DropView)
    }

    pub fn is_view(&self) -> bool {
        matches!(self, Self::DropView)
    }
}

#[derive(IntoElement)]
pub struct ConfirmDialog {
    kind: ConfirmActionKind,
    table: TableInfo,
    database_family: DatabaseFamily,
    sql_preview: String,
    language: AppLanguage,
    is_executing: bool,
    error_message: Option<String>,
    on_confirm: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_cancel: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl ConfirmDialog {
    pub fn new(
        kind: ConfirmActionKind,
        table: TableInfo,
        database_family: DatabaseFamily,
        sql_preview: String,
    ) -> Self {
        Self {
            kind,
            table,
            database_family,
            sql_preview,
            language: AppLanguage::En,
            is_executing: false,
            error_message: None,
            on_confirm: None,
            on_cancel: None,
        }
    }

    pub fn language(mut self, lang: AppLanguage) -> Self {
        self.language = lang;
        self
    }

    pub fn executing(mut self, executing: bool) -> Self {
        self.is_executing = executing;
        self
    }

    pub fn error(mut self, error: Option<String>) -> Self {
        self.error_message = error;
        self
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

impl RenderOnce for ConfirmDialog {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let lang = self.language;
        let is_drop = self.kind.is_drop();
        let is_view = self.kind.is_view();

        let title = match self.kind {
            ConfirmActionKind::DropTable => t("dialog.drop_table_title", lang),
            ConfirmActionKind::DropView => t("dialog.drop_view_title", lang),
            ConfirmActionKind::TruncateTable => t("dialog.truncate_table_title", lang),
        };

        let desc = match self.kind {
            ConfirmActionKind::DropTable => t("dialog.drop_table_desc", lang),
            ConfirmActionKind::DropView => t("dialog.drop_view_desc", lang),
            ConfirmActionKind::TruncateTable => t("dialog.truncate_table_desc", lang),
        };

        let confirm_btn_label = match self.kind {
            ConfirmActionKind::DropTable => t("dialog.confirm_drop", lang),
            ConfirmActionKind::DropView => t("dialog.confirm_drop_view", lang),
            ConfirmActionKind::TruncateTable => t("dialog.confirm_truncate", lang),
        };

        let on_confirm = self.on_confirm.clone();
        let on_cancel = self.on_cancel.clone();
        let on_cancel_hdr = self.on_cancel.clone();

        let cancel_btn = {
            let mut btn = Button::new("confirm_dialog_cancel_btn")
                .ghost()
                .small()
                .label(t("dialog.cancel", lang));
            if let Some(ref handler) = on_cancel {
                let handler = handler.clone();
                btn = btn.on_click(move |_, window, cx| {
                    handler(window, cx);
                });
            }
            btn
        };

        let header_close_btn = {
            let mut btn = Button::new("confirm_dialog_close_x")
                .ghost()
                .xsmall()
                .icon(IconName::X);
            if let Some(ref handler) = on_cancel_hdr {
                let handler = handler.clone();
                btn = btn.on_click(move |_, window, cx| {
                    handler(window, cx);
                });
            }
            btn
        };

        let action_btn = {
            let mut btn = Button::new("confirm_dialog_action_btn")
                .small()
                .disabled(self.is_executing);

            if is_drop {
                btn = btn.danger();
            } else {
                btn = btn.primary();
            }

            if self.is_executing {
                btn = btn
                    .icon(IconName::RotateCw)
                    .label(t("console.running", lang));
            } else {
                btn = btn
                    .icon(if is_drop {
                        IconName::Trash
                    } else {
                        IconName::RotateCcw
                    })
                    .label(confirm_btn_label);
            }

            if let Some(ref handler) = on_confirm {
                let handler = handler.clone();
                btn = btn.on_click(move |_, window, cx| {
                    handler(window, cx);
                });
            }
            btn
        };

        let header_icon_name = if is_drop {
            IconName::Trash
        } else {
            IconName::TriangleAlert
        };
        let header_icon_color = if is_drop {
            ThemeColors::ERROR
        } else {
            ThemeColors::WARNING
        };

        let table_target_badge = h_flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .child(
                Icon::new(if is_view {
                    IconName::Eye
                } else {
                    IconName::Table
                })
                .size(px(16.0))
                .text_color(ThemeColors::PRIMARY_BORDER),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(self.table.qualified_name(self.database_family)),
            )
            .child(
                div()
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .bg(ThemeColors::BG_APP)
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(if is_view { "VIEW" } else { "TABLE" }),
            )
            .child(
                div()
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .bg(ThemeColors::BG_APP)
                    .text_xs()
                    .text_color(ThemeColors::PRIMARY_BORDER)
                    .child(format!("{:?}", self.database_family)),
            );

        let error_banner = self.error_message.map(|err| {
            h_flex()
                .w_full()
                .items_start()
                .gap_2()
                .p_2p5()
                .rounded_md()
                .bg(rgba(0xEF444418))
                .border_1()
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
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::ERROR)
                                .child("Execution Failed"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_family("JetBrains Mono")
                                .text_color(ThemeColors::ERROR)
                                .child(err),
                        ),
                )
        });

        let modal = v_flex()
            .w(px(520.0))
            .max_w(px(560.0))
            .rounded_xl()
            .border_1()
            .border_color(ThemeColors::BORDER_PROMINENT)
            .bg(ThemeColors::BG_APP)
            .shadow_lg()
            .overflow_hidden()
            .child(
                // Modal Header
                h_flex()
                    .h(px(52.0))
                    .w_full()
                    .px_5()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                Icon::new(header_icon_name)
                                    .size(px(18.0))
                                    .text_color(header_icon_color),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(title),
                            ),
                    )
                    .child(header_close_btn),
            )
            .child(
                // Modal Body
                v_flex()
                    .p_5()
                    .gap_3p5()
                    .child(table_target_badge)
                    .child(
                        h_flex()
                            .w_full()
                            .items_start()
                            .gap_2p5()
                            .p_3()
                            .rounded_md()
                            .bg(if is_drop {
                                rgba(0xEF444415)
                            } else {
                                rgba(0xF59E0B15)
                            })
                            .border_1()
                            .border_color(if is_drop {
                                rgba(0xEF444435)
                            } else {
                                rgba(0xF59E0B35)
                            })
                            .child(
                                div().flex_shrink_0().pt_0p5().child(
                                    Icon::new(IconName::TriangleAlert)
                                        .size(px(16.0))
                                        .text_color(if is_drop {
                                            ThemeColors::ERROR
                                        } else {
                                            ThemeColors::WARNING
                                        }),
                                ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .line_height(px(18.0))
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(desc),
                            ),
                    )
                    .children(error_banner)
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("dialog.sql_statement", lang)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .p_3()
                                    .rounded_md()
                                    .bg(ThemeColors::BG_SURFACE)
                                    .border_1()
                                    .border_color(ThemeColors::BORDER)
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(ThemeColors::PRIMARY_BORDER)
                                    .child(self.sql_preview),
                            ),
                    ),
            )
            .child(
                // Modal Footer
                h_flex()
                    .h(px(52.0))
                    .w_full()
                    .px_5()
                    .justify_between()
                    .items_center()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child("Press Esc to cancel"),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(cancel_btn)
                            .child(action_btn),
                    ),
            );

        div()
            .id("confirm_dialog_backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000B0))
            .justify_center()
            .items_center()
            .flex()
            .child(modal)
    }
}
