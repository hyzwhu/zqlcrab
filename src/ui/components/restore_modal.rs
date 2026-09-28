//! SQL Script Restore & Batch Runner Modal Dialog.
//! Parses and executes local `.sql` scripts with static inspection, statement splitting,
//! error isolation policies, real-time progress indicators, and live execution audit logging.

use crate::db::restore::{
    RestoreErrorPolicy, RestoreOptions, RestoreProgress, RestoreSummary, SqlScriptAnalysis,
    StatementExecutionLog,
};
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    spinner::Spinner,
};
use gpui_kit::gpui::{
    AnyElement, App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, px, rgba,
};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestoreModalStep {
    #[default]
    SelectFile,
    Options,
    Execution,
}

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct RestoreModal {
    step: RestoreModalStep,
    file_path: Option<String>,
    analysis: Option<SqlScriptAnalysis>,
    options: RestoreOptions,
    is_executing: bool,
    progress: Option<RestoreProgress>,
    logs: Vec<StatementExecutionLog>,
    summary: Option<RestoreSummary>,
    error_msg: Option<String>,
    language: AppLanguage,

    on_step_change: Option<Rc<dyn Fn(RestoreModalStep, &mut Window, &mut App) + 'static>>,
    on_browse_file: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_change_policy: Option<Rc<dyn Fn(RestoreErrorPolicy, &mut Window, &mut App) + 'static>>,
    on_toggle_transaction: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_fk_checks: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_start_restore: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl RestoreModal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        step: RestoreModalStep,
        file_path: Option<String>,
        analysis: Option<SqlScriptAnalysis>,
        options: RestoreOptions,
        is_executing: bool,
        progress: Option<RestoreProgress>,
        logs: Vec<StatementExecutionLog>,
        summary: Option<RestoreSummary>,
        error_msg: Option<String>,
        language: AppLanguage,
    ) -> Self {
        Self {
            step,
            file_path,
            analysis,
            options,
            is_executing,
            progress,
            logs,
            summary,
            error_msg,
            language,
            on_step_change: None,
            on_browse_file: None,
            on_change_policy: None,
            on_toggle_transaction: None,
            on_toggle_fk_checks: None,
            on_start_restore: None,
            on_close: None,
        }
    }

    pub fn on_step_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(RestoreModalStep, &mut Window, &mut App) + 'static,
    {
        self.on_step_change = Some(Rc::new(handler));
        self
    }

    pub fn on_browse_file<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_browse_file = Some(Rc::new(handler));
        self
    }

    pub fn on_change_policy<F>(mut self, handler: F) -> Self
    where
        F: Fn(RestoreErrorPolicy, &mut Window, &mut App) + 'static,
    {
        self.on_change_policy = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_transaction<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_transaction = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_fk_checks<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_fk_checks = Some(Rc::new(handler));
        self
    }

    pub fn on_start_restore<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_start_restore = Some(Rc::new(handler));
        self
    }

    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    fn render_step_tab(
        &self,
        step: RestoreModalStep,
        label: &'static str,
        icon: IconName,
    ) -> impl IntoElement {
        let is_active = self.step == step;
        let handler = self.on_step_change.clone();

        let mut btn = Button::new(match step {
            RestoreModalStep::SelectFile => "restore_step_tab_file",
            RestoreModalStep::Options => "restore_step_tab_opts",
            RestoreModalStep::Execution => "restore_step_tab_exec",
        })
        .small()
        .icon(icon)
        .label(label);

        if is_active {
            btn = btn.primary();
        } else {
            btn = btn.ghost();
        }

        if !self.is_executing
            && let Some(h) = handler
        {
            btn = btn.on_click(move |_, window, cx| {
                h(step, window, cx);
            });
        }

        btn
    }
}

impl RenderOnce for RestoreModal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let lang = self.language;
        let on_close_click = self.on_close.clone();

        let header = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .p_4()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::FileText)
                                    .size(px(18.0))
                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("restore_modal.title", lang)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("restore_modal.subtitle", lang)),
                    ),
            )
            .child(
                Button::new("restore_modal_close_btn")
                    .ghost()
                    .small()
                    .icon(IconName::X)
                    .on_click(move |_, window, cx| {
                        if let Some(ref h) = on_close_click {
                            h(window, cx);
                        }
                    }),
            );

        // Navigation Tabs
        let step_nav = h_flex()
            .w_full()
            .px_4()
            .py_2()
            .gap_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(self.render_step_tab(
                RestoreModalStep::SelectFile,
                t("restore_modal.select_file", lang),
                IconName::Folder,
            ))
            .child(self.render_step_tab(
                RestoreModalStep::Options,
                t("dump_modal.step_options", lang),
                IconName::Settings,
            ))
            .child(self.render_step_tab(
                RestoreModalStep::Execution,
                t("restore_modal.live_logs", lang),
                IconName::Terminal,
            ));

        let body: AnyElement = match self.step {
            RestoreModalStep::SelectFile => self.render_select_file_step(lang).into_any_element(),
            RestoreModalStep::Options => self.render_options_step(lang).into_any_element(),
            RestoreModalStep::Execution => self.render_execution_step(lang).into_any_element(),
        };

        let footer = self.render_footer(lang);

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000088))
            .items_center()
            .justify_center()
            .flex()
            .child(
                v_flex()
                    .w(px(720.0))
                    .max_h(px(600.0))
                    .bg(ThemeColors::BG_APP)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .rounded_lg()
                    .shadow_lg()
                    .overflow_hidden()
                    .child(header)
                    .child(step_nav)
                    .child(body)
                    .child(footer),
            )
    }
}

impl RestoreModal {
    fn render_select_file_step(&self, lang: AppLanguage) -> impl IntoElement {
        let on_browse = self.on_browse_file.clone();
        let path_text = self
            .file_path
            .clone()
            .unwrap_or_else(|| t("restore_modal.no_file", lang).to_string());

        let file_picker = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .p_3()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .child(
                Icon::new(IconName::FileText)
                    .size(px(16.0))
                    .text_color(ThemeColors::PRIMARY_LIGHT),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(if self.file_path.is_some() {
                        ThemeColors::TEXT_PRIMARY
                    } else {
                        ThemeColors::TEXT_MUTED
                    })
                    .child(path_text),
            )
            .child(
                Button::new("restore_browse_btn")
                    .outline()
                    .small()
                    .icon(IconName::Folder)
                    .label(t("restore_modal.browse", lang))
                    .on_click(move |_, window, cx| {
                        if let Some(ref h) = on_browse {
                            h(window, cx);
                        }
                    }),
            );

        let mut content = v_flex().size_full().p_4().gap_3().child(file_picker);

        // Analysis Cards
        if let Some(ref ana) = self.analysis {
            let metrics_row = h_flex()
                .w_full()
                .gap_2()
                .child(
                    v_flex()
                        .flex_1()
                        .p_2p5()
                        .rounded_md()
                        .bg(ThemeColors::BG_SURFACE)
                        .border_1()
                        .border_color(ThemeColors::BORDER)
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(t("restore_modal.total_stmts", lang)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(ana.total_statements.to_string()),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .p_2p5()
                        .rounded_md()
                        .bg(ThemeColors::BG_SURFACE)
                        .border_1()
                        .border_color(ThemeColors::BORDER)
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(t("restore_modal.ddl_stmts", lang)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::PRIMARY_LIGHT)
                                .child(ana.ddl_count.to_string()),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .p_2p5()
                        .rounded_md()
                        .bg(ThemeColors::BG_SURFACE)
                        .border_1()
                        .border_color(ThemeColors::BORDER)
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(t("restore_modal.dml_stmts", lang)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::SUCCESS)
                                .child(ana.dml_count.to_string()),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .p_2p5()
                        .rounded_md()
                        .bg(ThemeColors::BG_SURFACE)
                        .border_1()
                        .border_color(ThemeColors::BORDER)
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(t("restore_modal.file_size", lang)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(format!("{} KB", ana.file_size_bytes / 1024)),
                        ),
                );

            let mut preview_list = v_flex().gap_1();
            for (idx, p) in ana.preview_statements.iter().enumerate() {
                preview_list = preview_list.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(ThemeColors::BG_SURFACE)
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .w(px(24.0))
                                .child(format!("{}.", idx + 1)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .font_weight(FontWeight::NORMAL)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(p.clone()),
                        ),
                );
            }

            let preview_panel = v_flex()
                .id("restore_preview_scroll")
                .flex_1()
                .p_3()
                .gap_2()
                .rounded_md()
                .bg(ThemeColors::BG_SURFACE)
                .border_1()
                .border_color(ThemeColors::BORDER)
                .overflow_y_scroll()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(t("restore_modal.preview_title", lang)),
                )
                .child(preview_list);

            content = content.child(metrics_row).child(preview_panel);
        }

        content
    }

    #[allow(clippy::type_complexity)]
    fn render_options_step(&self, lang: AppLanguage) -> impl IntoElement {
        let cur_policy = self.options.error_policy;
        let on_pol = self.on_change_policy.clone();

        let render_policy_btn = |policy: RestoreErrorPolicy, id: &'static str, label: &'static str| {
            let is_sel = cur_policy == policy;
            let on_pol_click = on_pol.clone();

            let mut btn = Button::new(id)
                .small()
                .w_full()
                .icon(if is_sel {
                    IconName::Check
                } else {
                    IconName::Circle
                })
                .label(label);

            if is_sel {
                btn = btn.primary();
            } else {
                btn = btn.outline();
            }

            if let Some(h) = on_pol_click {
                btn = btn.on_click(move |_, window, cx| {
                    h(policy, window, cx);
                });
            }
            btn
        };

        let on_tx = self.on_toggle_transaction.clone();
        let cur_tx = self.options.wrap_in_transaction;
        let on_fk = self.on_toggle_fk_checks.clone();
        let cur_fk = self.options.disable_foreign_keys;

        let render_checkbox_btn = |id: &'static str,
                                   is_checked: bool,
                                   label: &'static str,
                                   handler: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>| {
            let next_val = !is_checked;
            let mut btn = Button::new(id)
                .small()
                .w_full()
                .icon(if is_checked {
                    IconName::Check
                } else {
                    IconName::Circle
                })
                .label(label);

            if is_checked {
                btn = btn.primary();
            } else {
                btn = btn.outline();
            }

            if let Some(h) = handler {
                btn = btn.on_click(move |_, window, cx| {
                    h(next_val, window, cx);
                });
            }
            btn
        };

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(t("restore_modal.policy_label", lang)),
            )
            .child(render_policy_btn(
                RestoreErrorPolicy::StopOnError,
                "policy_stop_btn",
                t("restore_modal.policy_stop", lang),
            ))
            .child(render_policy_btn(
                RestoreErrorPolicy::ContinueAndLog,
                "policy_continue_btn",
                t("restore_modal.policy_continue", lang),
            ))
            .child(render_checkbox_btn(
                "chk_restore_tx",
                cur_tx,
                t("restore_modal.opt_transaction", lang),
                on_tx,
            ))
            .child(render_checkbox_btn(
                "chk_restore_fk",
                cur_fk,
                t("dump_modal.opt_fk_checks", lang),
                on_fk,
            ))
    }

    fn render_execution_step(&self, lang: AppLanguage) -> impl IntoElement {
        let mut content = v_flex().size_full().p_4().gap_3();

        // Progress Bar Card
        if let Some(ref prog) = self.progress {
            let prog_card = v_flex()
                .w_full()
                .p_3()
                .gap_2()
                .rounded_md()
                .bg(ThemeColors::BG_SURFACE)
                .border_1()
                .border_color(ThemeColors::BORDER)
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(Spinner::new())
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(ThemeColors::PRIMARY_LIGHT)
                                        .child(format!(
                                            "Statement {} of {} ({:.1}%)",
                                            prog.current_statement,
                                            prog.total_statements,
                                            prog.percent
                                        )),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_3()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::SUCCESS)
                                        .child(format!("✓ {}", prog.succeeded_count)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::ERROR)
                                        .child(format!("✗ {}", prog.failed_count)),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(prog.current_sql_preview.clone()),
                );
            content = content.child(prog_card);
        }

        // Summary Card if Completed
        if let Some(ref sum) = self.summary {
            let status_c = if sum.failed_count == 0 {
                ThemeColors::SUCCESS
            } else {
                ThemeColors::WARNING
            };
            let status_title = if sum.aborted_early {
                t("restore_modal.aborted", lang)
            } else {
                t("restore_modal.completed", lang)
            };

            let sum_card = v_flex()
                .w_full()
                .p_3()
                .gap_1p5()
                .rounded_md()
                .bg(rgba(0x28a74515))
                .border_1()
                .border_color(status_c)
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(if sum.failed_count == 0 {
                                IconName::Check
                            } else {
                                IconName::TriangleAlert
                            })
                            .size(px(16.0))
                            .text_color(status_c),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(status_c)
                                .child(status_title),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(format!(
                            "Total: {} | Succeeded: {} | Failed: {} | Time: {} ms",
                            sum.total_statements,
                            sum.succeeded_count,
                            sum.failed_count,
                            sum.duration_ms
                        )),
                );
            content = content.child(sum_card);
        }

        // Logs Scroll Container
        let mut logs_list = v_flex().gap_1();
        if self.logs.is_empty() {
            logs_list = logs_list.child(
                div()
                    .p_6()
                    .text_center()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child("No execution events logged yet"),
            );
        } else {
            for log in &self.logs {
                let is_ok = log.is_success;
                let log_row = h_flex()
                    .w_full()
                    .px_2p5()
                    .py_1p5()
                    .gap_2()
                    .items_start()
                    .rounded_sm()
                    .bg(if is_ok {
                        rgba(0x00000020)
                    } else {
                        rgba(0xdc354515)
                    })
                    .child(
                        Icon::new(if is_ok { IconName::Check } else { IconName::X })
                            .size(px(12.0))
                            .text_color(if is_ok {
                                ThemeColors::SUCCESS
                            } else {
                                ThemeColors::ERROR
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("#{}", log.index + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(log.sql_preview.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("{}ms", log.duration_ms)),
                    );

                logs_list = logs_list.child(log_row);

                if let Some(ref err) = log.error_message {
                    logs_list = logs_list.child(
                        div()
                            .pl_6()
                            .pr_2()
                            .pb_1()
                            .text_size(px(11.0))
                            .text_color(ThemeColors::ERROR)
                            .child(err.clone()),
                    );
                }
            }
        }

        let logs_panel = v_flex()
            .id("restore_logs_scroll")
            .flex_1()
            .p_3()
            .gap_2()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .overflow_y_scroll()
            .child(logs_list);

        content = content.child(logs_panel);

        if let Some(ref err) = self.error_msg {
            let err_card = div()
                .p_2()
                .rounded_md()
                .bg(rgba(0xdc354515))
                .border_1()
                .border_color(ThemeColors::ERROR)
                .text_xs()
                .text_color(ThemeColors::ERROR)
                .child(err.clone());
            content = content.child(err_card);
        }

        content
    }

    fn render_footer(&self, lang: AppLanguage) -> impl IntoElement {
        let on_close_click = self.on_close.clone();
        let on_start = self.on_start_restore.clone();
        let is_running = self.is_executing;
        let no_file = self.file_path.is_none();

        let mut restore_btn = Button::new("restore_start_btn")
            .primary()
            .small()
            .icon(IconName::Play)
            .label(if is_running {
                t("restore_modal.executing", lang)
            } else {
                t("restore_modal.btn_restore", lang)
            });

        if is_running || no_file {
            // Disabled
        } else if let Some(ref h) = on_start {
            let h = h.clone();
            restore_btn = restore_btn.on_click(move |_, window, cx| {
                h(window, cx);
            });
        }

        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_4()
            .py_3()
            .bg(ThemeColors::BG_SURFACE)
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .child(
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(if no_file {
                        "Please select a SQL script file to run"
                    } else {
                        ""
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("restore_cancel_btn")
                            .outline()
                            .small()
                            .label(t("common.close", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_close_click {
                                    h(window, cx);
                                }
                            }),
                    )
                    .child(restore_btn),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_modal_initialization() {
        let modal = RestoreModal::new(
            RestoreModalStep::SelectFile,
            Some("backup.sql".to_string()),
            None,
            RestoreOptions::default(),
            false,
            None,
            Vec::new(),
            None,
            None,
            AppLanguage::En,
        );

        assert_eq!(modal.step, RestoreModalStep::SelectFile);
        assert_eq!(modal.file_path.as_deref(), Some("backup.sql"));
        assert_eq!(modal.options.error_policy, RestoreErrorPolicy::StopOnError);
    }
}
