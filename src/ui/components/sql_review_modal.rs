//! SQL Review & Confirmation modal dialog shown before executing pending updates and deletions.

use crate::db::sql_gen::SqlReviewPlan;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::gpui::{
    prelude::*,
    App, FontWeight, IntoElement, ParentElement, RenderOnce,
    Styled, Window, div, px, rgba,
};
use std::rc::Rc;

type ActionCallback = Rc<dyn Fn(&mut Window, &mut App)>;
type CopyCallback = Rc<dyn Fn(String, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct SqlReviewModal {
    plan: SqlReviewPlan,
    is_executing: bool,
    error_message: Option<String>,
    copied: bool,
    on_execute: Option<ActionCallback>,
    on_cancel: Option<ActionCallback>,
    on_copy: Option<CopyCallback>,
}

impl SqlReviewModal {
    pub fn new(plan: SqlReviewPlan) -> Self {
        Self {
            plan,
            is_executing: false,
            error_message: None,
            copied: false,
            on_execute: None,
            on_cancel: None,
            on_copy: None,
        }
    }

    pub fn executing(mut self, is_executing: bool) -> Self {
        self.is_executing = is_executing;
        self
    }

    pub fn error(mut self, error: Option<String>) -> Self {
        self.error_message = error;
        self
    }

    pub fn copied(mut self, copied: bool) -> Self {
        self.copied = copied;
        self
    }

    pub fn on_execute<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_execute = Some(Rc::new(handler));
        self
    }

    pub fn on_cancel<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    pub fn on_copy<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SqlReviewModal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let cancel_handler = self.on_cancel.clone();
        let execute_handler = self.on_execute.clone();
        let copy_handler = self.on_copy.clone();

        let mut close_btn = Button::new("close_review_modal")
            .ghost()
            .xsmall()
            .icon(IconName::X);
        if let Some(ref on_cancel) = cancel_handler {
            let on_cancel = on_cancel.clone();
            close_btn = close_btn.on_click(move |_, window, cx| {
                on_cancel(window, cx);
            });
        }

        let mut cancel_btn = Button::new("cancel_review_btn")
            .ghost()
            .small()
            .label("Cancel");
        if let Some(ref on_cancel) = cancel_handler {
            let on_cancel = on_cancel.clone();
            cancel_btn = cancel_btn.on_click(move |_, window, cx| {
                on_cancel(window, cx);
            });
        }

        let mut exec_btn = Button::new("confirm_exec_btn")
            .primary()
            .small()
            .icon(if self.is_executing {
                IconName::Loader
            } else {
                IconName::Check
            })
            .label(if self.is_executing {
                "Executing Changes..."
            } else {
                "Execute Changes (⌘↵)"
            });
        if !self.is_executing {
            if let Some(ref on_exec) = execute_handler {
                let on_exec = on_exec.clone();
                exec_btn = exec_btn.on_click(move |_, window, cx| {
                    on_exec(window, cx);
                });
            }
        }

        let script_to_copy = self.plan.full_script.clone();
        let mut copy_btn = Button::new("copy_sql_btn")
            .outline()
            .xsmall()
            .icon(if self.copied {
                IconName::Check
            } else {
                IconName::Copy
            })
            .label(if self.copied { "Copied!" } else { "Copy SQL" });
        if let Some(ref on_copy) = copy_handler {
            let on_copy = on_copy.clone();
            copy_btn = copy_btn.on_click(move |_, window, cx| {
                on_copy(script_to_copy.clone(), window, cx);
            });
        }

        // Header table & engine badge
        let family_str = format!("{:?}", self.plan.database_family);
        let table_display = match &self.plan.schema_name {
            Some(s) if !s.is_empty() => format!("{}.{}", s, self.plan.table_name),
            _ => self.plan.table_name.clone(),
        };

        // Stats badges
        let inserts_count = self.plan.inserts_count;
        let updates_count = self.plan.updates_count;
        let deletes_count = self.plan.deletes_count;

        // Primary key safety info banner
        let pk_banner = if self.plan.has_primary_key {
            let pks = self.plan.primary_keys.join(", ");
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .p_2()
                .px_3()
                .rounded_md()
                .bg(rgba(0x10B98115))
                .border_1()
                .border_color(rgba(0x10B98133))
                .child(
                    Icon::new(IconName::Check)
                        .size(px(14.0))
                        .text_color(ThemeColors::SUCCESS),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(format!("Safely targeted using Primary Key: [{pks}]")),
                )
        } else {
            h_flex()
                .w_full()
                .items_start()
                .gap_2()
                .p_2()
                .px_3()
                .rounded_md()
                .bg(rgba(0xF59E0B15))
                .border_1()
                .border_color(rgba(0xF59E0B33))
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size(px(14.0))
                        .text_color(ThemeColors::WARNING),
                )
                .child(
                    v_flex()
                        .gap_0p5()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::WARNING)
                                .child("No Primary Key Detected"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child("Conditions will match all original column values. If duplicate identical rows exist in the table, all matching rows will be affected."),
                        ),
                )
        };

        // Execution error alert banner if present
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
                    Icon::new(IconName::TriangleAlert)
                        .size(px(14.0))
                        .text_color(ThemeColors::ERROR),
                )
                .child(
                    v_flex()
                        .gap_0p5()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::ERROR)
                                .child("Execution Failed (Transaction Rolled Back)"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::ERROR)
                                .child(err),
                        ),
                )
        });

        // Formatted code script block with line numbering
        let lines: Vec<&str> = self.plan.full_script.lines().collect();
        let code_lines = v_flex()
            .w_full()
            .gap_0p5()
            .children(lines.into_iter().enumerate().map(|(idx, line)| {
                let is_comment = line.trim_start().starts_with("--");
                let is_keyword = line.starts_with("BEGIN")
                    || line.starts_with("COMMIT")
                    || line.starts_with("START TRANSACTION");
                let is_insert = line.starts_with("INSERT INTO") || line.starts_with("INSERT");

                let text_color = if is_comment {
                    ThemeColors::TEXT_MUTED
                } else if is_keyword {
                    ThemeColors::PRIMARY_BORDER
                } else if is_insert {
                    ThemeColors::SUCCESS
                } else if line.starts_with("UPDATE") || line.starts_with("DELETE") {
                    ThemeColors::PRIMARY_LIGHT
                } else {
                    ThemeColors::TEXT_PRIMARY
                };

                h_flex()
                    .min_w_full()
                    .w_auto()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .w(px(28.0))
                            .flex_none()
                            .text_right()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(format!("{}", idx + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(text_color)
                            .child(line.to_string()),
                    )
            }));

        // Modal container
        let modal = v_flex()
            .w(px(860.0))
            .max_w(px(960.0))
            .max_h(px(640.0))
            .bg(ThemeColors::BG_APP)
            .rounded_xl()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .shadow_lg()
            .child(
                // Modal header
                h_flex()
                    .w_full()
                    .p_4()
                    .justify_between()
                    .items_center()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Icon::new(IconName::FileCode)
                                    .size(px(20.0))
                                    .text_color(ThemeColors::PRIMARY_BORDER),
                            )
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("Review SQL Changes"),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_full()
                                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                                    .border_1()
                                                    .border_color(ThemeColors::BORDER)
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(ThemeColors::TEXT_MUTED)
                                                    .child(format!("{table_display} · {family_str}")),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child("Verify the generated atomic transaction script before committing changes to the database"),
                                    ),
                            ),
                    )
                    .child(close_btn),
            )
            .child(
                // Modal body
                v_flex()
                    .p_4()
                    .gap_3()
                    .flex_1()
                    .min_h_0()
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .when(inserts_count > 0, |this| {
                                        this.child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_md()
                                                .bg(rgba(0x10B98120))
                                                .border_1()
                                                .border_color(rgba(0x10B98140))
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(ThemeColors::SUCCESS)
                                                .child(format!("● {inserts_count} Insertion{}", if inserts_count > 1 { "s" } else { "" })),
                                        )
                                    })
                                    .when(updates_count > 0, |this| {
                                        this.child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_md()
                                                .bg(rgba(0x3B82F620))
                                                .border_1()
                                                .border_color(rgba(0x3B82F640))
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(ThemeColors::PRIMARY_LIGHT)
                                                .child(format!("● {updates_count} Update{}", if updates_count > 1 { "s" } else { "" })),
                                        )
                                    })
                                    .when(deletes_count > 0, |this| {
                                        this.child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_md()
                                                .bg(rgba(0xEF444420))
                                                .border_1()
                                                .border_color(rgba(0xEF444440))
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(ThemeColors::ERROR)
                                                .child(format!("● {deletes_count} Deletion{}", if deletes_count > 1 { "s" } else { "" })),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_FAINT)
                                            .child("Wrapped in single atomic transaction"),
                                    ),
                            )
                            .child(copy_btn),
                    )
                    .child(pk_banner)
                    .children(error_banner)
                    .child(
                        // SQL Script viewport with dual-axis scrollbar to prevent horizontal clipping
                        div()
                            .id("sql_review_code_scroll")
                            .w_full()
                            .max_h(px(340.0))
                            .overflow_x_scroll()
                            .overflow_y_scroll()
                            .p_3()
                            .rounded_md()
                            .bg(rgba(0x0B1120FF))
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .child(
                                div()
                                    .min_w_full()
                                    .w_auto()
                                    .child(code_lines),
                            ),
                    ),
            )
            .child(
                // Modal footer
                h_flex()
                    .w_full()
                    .p_3()
                    .px_4()
                    .justify_between()
                    .items_center()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child("Press Esc to dismiss · Changes remain staged"),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(cancel_btn)
                            .child(exec_btn),
                    ),
            );

        // Backdrop overlay
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000B0))
            .justify_center()
            .items_center()
            .flex()
            .child(modal)
    }
}
