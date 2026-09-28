//! Visual Cross-Database Data Transfer Wizard Modal Dialog.
//! Allows selecting source and target connections and databases, mapping tables,
//! configuring DDL and DML options, and running live transfer with real-time progress and audit logs.

use crate::db::transfer::{
    TransferAuditLog, TransferLogLevel, TransferOptions, TransferProgress, TransferScope,
    TransferSummary, TransferTableMapping,
};
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
    spinner::Spinner,
};
use gpui_kit::gpui::{
    Anchor, AnyElement, App, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, px, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransferModalStep {
    #[default]
    SourceTarget,
    TableMapping,
    Options,
    Execution,
}

#[derive(IntoElement)]
pub struct TransferModal {
    step: TransferModalStep,
    source_conn_id: Option<String>,
    source_conn_name: String,
    source_db: Option<String>,
    target_conn_id: Option<String>,
    target_conn_name: String,
    target_db: Option<String>,
    available_connections: Vec<(String, String)>,
    available_source_dbs: Vec<String>,
    available_target_dbs: Vec<String>,
    tables: Vec<TransferTableMapping>,
    filter_query: String,
    options: TransferOptions,
    is_running: bool,
    progress: Option<TransferProgress>,
    summary: Option<TransferSummary>,
    logs: Vec<TransferAuditLog>,
    error_msg: Option<String>,
    copied: bool,
    language: AppLanguage,

    // Event Callbacks
    on_step_change: Option<Rc<dyn Fn(TransferModalStep, &mut Window, &mut App) + 'static>>,
    on_select_source_conn: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_source_db: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_target_conn: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_target_db: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_toggle_table: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_cycle_table_scope: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_select_all: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_deselect_all: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_set_all_scopes: Option<Rc<dyn Fn(TransferScope, &mut Window, &mut App) + 'static>>,
    on_toggle_drop_target: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_create_target: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_truncate_target: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_disable_fk: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_transaction: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_continue_error: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_change_batch_size: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_start_transfer: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_abort_transfer: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_copy_logs: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl TransferModal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        step: TransferModalStep,
        source_conn_id: Option<String>,
        source_conn_name: String,
        source_db: Option<String>,
        target_conn_id: Option<String>,
        target_conn_name: String,
        target_db: Option<String>,
        available_connections: Vec<(String, String)>,
        available_source_dbs: Vec<String>,
        available_target_dbs: Vec<String>,
        tables: Vec<TransferTableMapping>,
        filter_query: String,
        options: TransferOptions,
        is_running: bool,
        progress: Option<TransferProgress>,
        summary: Option<TransferSummary>,
        logs: Vec<TransferAuditLog>,
        error_msg: Option<String>,
        copied: bool,
        language: AppLanguage,
    ) -> Self {
        Self {
            step,
            source_conn_id,
            source_conn_name,
            source_db,
            target_conn_id,
            target_conn_name,
            target_db,
            available_connections,
            available_source_dbs,
            available_target_dbs,
            tables,
            filter_query,
            options,
            is_running,
            progress,
            summary,
            logs,
            error_msg,
            copied,
            language,
            on_step_change: None,
            on_select_source_conn: None,
            on_select_source_db: None,
            on_select_target_conn: None,
            on_select_target_db: None,
            on_toggle_table: None,
            on_cycle_table_scope: None,
            on_select_all: None,
            on_deselect_all: None,
            on_set_all_scopes: None,
            on_toggle_drop_target: None,
            on_toggle_create_target: None,
            on_toggle_truncate_target: None,
            on_toggle_disable_fk: None,
            on_toggle_transaction: None,
            on_toggle_continue_error: None,
            on_change_batch_size: None,
            on_start_transfer: None,
            on_abort_transfer: None,
            on_copy_logs: None,
            on_close: None,
        }
    }

    pub fn on_step_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(TransferModalStep, &mut Window, &mut App) + 'static,
    {
        self.on_step_change = Some(Rc::new(handler));
        self
    }

    pub fn on_select_source_conn<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_source_conn = Some(Rc::new(handler));
        self
    }

    pub fn on_select_source_db<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_source_db = Some(Rc::new(handler));
        self
    }

    pub fn on_select_target_conn<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_target_conn = Some(Rc::new(handler));
        self
    }

    pub fn on_select_target_db<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_target_db = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_table = Some(Rc::new(handler));
        self
    }

    pub fn on_cycle_table_scope<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_cycle_table_scope = Some(Rc::new(handler));
        self
    }

    pub fn on_select_all<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_select_all = Some(Rc::new(handler));
        self
    }

    pub fn on_deselect_all<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_deselect_all = Some(Rc::new(handler));
        self
    }

    pub fn on_set_all_scopes<F>(mut self, handler: F) -> Self
    where
        F: Fn(TransferScope, &mut Window, &mut App) + 'static,
    {
        self.on_set_all_scopes = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_drop_target<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_drop_target = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_create_target<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_create_target = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_truncate_target<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_truncate_target = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_disable_fk<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_disable_fk = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_transaction<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_transaction = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_continue_error<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_continue_error = Some(Rc::new(handler));
        self
    }

    pub fn on_change_batch_size<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_change_batch_size = Some(Rc::new(handler));
        self
    }

    pub fn on_start_transfer<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_start_transfer = Some(Rc::new(handler));
        self
    }

    pub fn on_abort_transfer<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_abort_transfer = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_logs<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_copy_logs = Some(Rc::new(handler));
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
        step: TransferModalStep,
        label: &'static str,
        icon: IconName,
    ) -> impl IntoElement {
        let is_active = self.step == step;
        let handler = self.on_step_change.clone();

        let mut btn = Button::new(match step {
            TransferModalStep::SourceTarget => "transfer_step_tab_src_tgt",
            TransferModalStep::TableMapping => "transfer_step_tab_mapping",
            TransferModalStep::Options => "transfer_step_tab_options",
            TransferModalStep::Execution => "transfer_step_tab_execution",
        })
        .small()
        .icon(icon)
        .label(label);

        if is_active {
            btn = btn.primary();
        } else {
            btn = btn.ghost();
        }

        if !self.is_running
            && let Some(h) = handler
        {
            btn = btn.on_click(move |_, window, cx| {
                h(step, window, cx);
            });
        }

        btn
    }

    fn render_source_target_step(&self, lang: AppLanguage) -> impl IntoElement {
        let src_conn_handler = self.on_select_source_conn.clone();
        let src_db_handler = self.on_select_source_db.clone();
        let tgt_conn_handler = self.on_select_target_conn.clone();
        let tgt_db_handler = self.on_select_target_db.clone();

        let conns = self.available_connections.clone();
        let src_dbs = self.available_source_dbs.clone();
        let tgt_dbs = self.available_target_dbs.clone();

        let src_conn_label = if self.source_conn_name.is_empty() {
            "Select Connection...".to_string()
        } else {
            self.source_conn_name.clone()
        };

        let src_db_label = self.source_db.clone().unwrap_or_else(|| "Select Database...".to_string());

        let tgt_conn_label = if self.target_conn_name.is_empty() {
            "Select Connection...".to_string()
        } else {
            self.target_conn_name.clone()
        };

        let tgt_db_label = self.target_db.clone().unwrap_or_else(|| "Select Database...".to_string());

        let conns_clone1 = conns.clone();
        let src_conn_btn = Button::new("transfer_src_conn_btn")
            .small()
            .outline()
            .icon(IconName::Database)
            .child(src_conn_label)
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for (cid, cname) in &conns_clone1 {
                    let id = cid.clone();
                    let cb = src_conn_handler.clone();
                    menu = menu.item(PopupMenuItem::new(cname.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref h) = cb {
                                h(id.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        let src_db_btn = Button::new("transfer_src_db_btn")
            .small()
            .outline()
            .icon(IconName::Folder)
            .child(src_db_label)
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for db in &src_dbs {
                    let name = db.clone();
                    let cb = src_db_handler.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref h) = cb {
                                h(name.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        let conns_clone2 = conns.clone();
        let tgt_conn_btn = Button::new("transfer_tgt_conn_btn")
            .small()
            .outline()
            .icon(IconName::Database)
            .child(tgt_conn_label)
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for (cid, cname) in &conns_clone2 {
                    let id = cid.clone();
                    let cb = tgt_conn_handler.clone();
                    menu = menu.item(PopupMenuItem::new(cname.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref h) = cb {
                                h(id.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        let tgt_db_btn = Button::new("transfer_tgt_db_btn")
            .small()
            .outline()
            .icon(IconName::Folder)
            .child(tgt_db_label)
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for db in &tgt_dbs {
                    let name = db.clone();
                    let cb = tgt_db_handler.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref h) = cb {
                                h(name.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        let is_same_db = self.source_conn_id == self.target_conn_id
            && self.source_db.is_some()
            && self.target_db.is_some()
            && self.source_db == self.target_db;

        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .gap_4()
                    .child(
                        // Source Box
                        v_flex()
                            .flex_1()
                            .p_3()
                            .gap_2p5()
                            .rounded_md()
                            .bg(ThemeColors::BG_SURFACE)
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        Icon::new(IconName::Database)
                                            .size(px(14.0))
                                            .text_color(ThemeColors::PRIMARY_LIGHT),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(t("transfer_modal.source_db", lang)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("transfer_modal.source_conn", lang)),
                            )
                            .child(src_conn_btn)
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("transfer_modal.source_db", lang)),
                            )
                            .child(src_db_btn),
                    )
                    .child(
                        // Center Arrow
                        h_flex()
                            .items_center()
                            .justify_center()
                            .px_2()
                            .child(
                                Icon::new(IconName::ArrowRight)
                                    .size(px(22.0))
                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                            ),
                    )
                    .child(
                        // Target Box
                        v_flex()
                            .flex_1()
                            .p_3()
                            .gap_2p5()
                            .rounded_md()
                            .bg(ThemeColors::BG_SURFACE)
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        Icon::new(IconName::Database)
                                            .size(px(14.0))
                                            .text_color(ThemeColors::SUCCESS),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(t("transfer_modal.target_db", lang)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("transfer_modal.target_conn", lang)),
                            )
                            .child(tgt_conn_btn)
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("transfer_modal.target_db", lang)),
                            )
                            .child(tgt_db_btn),
                    ),
            )
            .when(is_same_db, |this| {
                this.child(
                    h_flex()
                        .w_full()
                        .p_2p5()
                        .gap_2()
                        .rounded_md()
                        .bg(rgba(0xd2992220))
                        .border_1()
                        .border_color(rgba(0xd2992260))
                        .items_center()
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size(px(14.0))
                                .text_color(rgba(0xd29922ff)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(t("transfer_modal.same_source_target_warning", lang)),
                        ),
                )
            })
            .when_some(self.error_msg.clone(), |this, err| {
                this.child(
                    h_flex()
                        .w_full()
                        .p_2p5()
                        .gap_2()
                        .rounded_md()
                        .bg(rgba(0xff444420))
                        .border_1()
                        .border_color(ThemeColors::ERROR)
                        .items_center()
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size(px(14.0))
                                .text_color(ThemeColors::ERROR),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::ERROR)
                                .child(err),
                        ),
                )
            })
    }

    fn render_table_mapping_step(&self, lang: AppLanguage) -> impl IntoElement {
        let on_sel_all = self.on_select_all.clone();
        let on_desel_all = self.on_deselect_all.clone();
        let on_set_scopes = self.on_set_all_scopes.clone();
        let on_toggle = self.on_toggle_table.clone();
        let on_cycle_scope = self.on_cycle_table_scope.clone();

        let total_count = self.tables.len();
        let selected_count = self.tables.iter().filter(|t| t.selected).count();

        let set_scope_fn = |scope: TransferScope| {
            let cb = on_set_scopes.clone();
            move |_: &gpui_kit::gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                if let Some(ref h) = cb {
                    h(scope, window, cx);
                }
            }
        };

        let toolbar = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_3()
            .py_2()
            .bg(ThemeColors::BG_SURFACE)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(format!(
                        "{selected_count} / {total_count} {}",
                        t("dump_modal.selected_count", lang)
                    )),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .child(
                        Button::new("transfer_sel_all_btn")
                            .outline()
                            .xsmall()
                            .label(t("transfer_modal.select_all", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_sel_all {
                                    h(window, cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("transfer_desel_all_btn")
                            .outline()
                            .xsmall()
                            .label(t("transfer_modal.deselect_all", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_desel_all {
                                    h(window, cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("transfer_set_full_btn")
                            .ghost()
                            .xsmall()
                            .label(t("transfer_modal.scope_all_full", lang))
                            .on_click(set_scope_fn(TransferScope::StructureAndData)),
                    )
                    .child(
                        Button::new("transfer_set_schema_btn")
                            .ghost()
                            .xsmall()
                            .label(t("transfer_modal.scope_all_schema", lang))
                            .on_click(set_scope_fn(TransferScope::StructureOnly)),
                    )
                    .child(
                        Button::new("transfer_set_data_btn")
                            .ghost()
                            .xsmall()
                            .label(t("transfer_modal.scope_all_data", lang))
                            .on_click(set_scope_fn(TransferScope::DataOnly)),
                    ),
            );

        let query = self.filter_query.to_lowercase();
        let mut list = v_flex().w_full().gap_1().p_2();

        for (idx, mapping) in self.tables.iter().enumerate() {
            if !query.is_empty()
                && !mapping.source_table.to_lowercase().contains(&query)
                && !mapping.target_table.to_lowercase().contains(&query)
            {
                continue;
            }

            let is_sel = mapping.selected;
            let cb_toggle = on_toggle.clone();
            let cb_scope = on_cycle_scope.clone();

            let scope_badge = Button::new(match mapping.scope {
                TransferScope::StructureAndData => "scope_struct_data",
                TransferScope::StructureOnly => "scope_struct_only",
                TransferScope::DataOnly => "scope_data_only",
            })
            .xsmall()
            .outline()
            .label(mapping.scope.display_name())
            .on_click(move |_, window, cx| {
                if let Some(ref h) = cb_scope {
                    h(idx, window, cx);
                }
            });

            let row_item = h_flex()
                .w_full()
                .h(px(32.0))
                .px_2()
                .items_center()
                .justify_between()
                .rounded_sm()
                .bg(if is_sel {
                    ThemeColors::BG_SURFACE_HOVER
                } else {
                    ThemeColors::TRANSPARENT
                })
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new(if is_sel { "chk_on" } else { "chk_off" })
                                .ghost()
                                .xsmall()
                                .icon(if is_sel {
                                    IconName::Check
                                } else {
                                    IconName::Circle
                                })
                                .on_click(move |_, window, cx| {
                                    if let Some(ref h) = cb_toggle {
                                        h(idx, window, cx);
                                    }
                                }),
                        )
                        .child(
                            Icon::new(IconName::Table)
                                .size(px(14.0))
                                .text_color(ThemeColors::TEXT_MUTED),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if is_sel {
                                    ThemeColors::TEXT_PRIMARY
                                } else {
                                    ThemeColors::TEXT_MUTED
                                })
                                .child(mapping.source_table.clone()),
                        )
                        .child(
                            Icon::new(IconName::ArrowRight)
                                .size(px(12.0))
                                .text_color(ThemeColors::TEXT_MUTED),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_sel {
                                    ThemeColors::PRIMARY_LIGHT
                                } else {
                                    ThemeColors::TEXT_MUTED
                                })
                                .child(mapping.target_table.clone()),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .when_some(mapping.source_row_count, |this, count| {
                            this.child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!("~{count} rows")),
                            )
                        })
                        .child(scope_badge),
                );

            list = list.child(row_item);
        }

        v_flex()
            .size_full()
            .child(toolbar)
            .child(
                v_flex()
                    .id("transfer_table_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list),
            )
    }

    fn render_options_step(&self, lang: AppLanguage) -> impl IntoElement {
        let on_drop = self.on_toggle_drop_target.clone();
        let cur_drop = self.options.drop_target_if_exists;
        let on_create = self.on_toggle_create_target.clone();
        let cur_create = self.options.create_target_if_not_exists;
        let on_truncate = self.on_toggle_truncate_target.clone();
        let cur_truncate = self.options.truncate_target_first;
        let on_fk = self.on_toggle_disable_fk.clone();
        let cur_fk = self.options.disable_foreign_keys;
        let on_tx = self.on_toggle_transaction.clone();
        let cur_tx = self.options.wrap_in_transaction;
        let on_cont = self.on_toggle_continue_error.clone();
        let cur_cont = self.options.continue_on_error;

        let on_batch = self.on_change_batch_size.clone();
        let cur_batch = self.options.batch_size;

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
                btn = btn.on_click(move |_, window, cx| h(next_val, window, cx));
            }

            btn
        };

        let batch_sizes = [100, 200, 500, 1000];
        let mut batch_btns = h_flex().gap_2();
        for b in batch_sizes {
            let is_sel = cur_batch == b;
            let mut b_btn = Button::new(match b {
                100 => "batch_100",
                200 => "batch_200",
                500 => "batch_500",
                _ => "batch_1000",
            })
            .small()
            .label(format!("{b} rows"));

            if is_sel {
                b_btn = b_btn.primary();
            } else {
                b_btn = b_btn.outline();
            }

            if let Some(ref h) = on_batch {
                let cb = h.clone();
                b_btn = b_btn.on_click(move |_, window, cx| cb(b, window, cx));
            }
            batch_btns = batch_btns.child(b_btn);
        }

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(render_checkbox_btn(
                "chk_transfer_create",
                cur_create,
                t("transfer_modal.opt_create_target", lang),
                on_create,
            ))
            .child(render_checkbox_btn(
                "chk_transfer_drop",
                cur_drop,
                t("transfer_modal.opt_drop_target", lang),
                on_drop,
            ))
            .child(render_checkbox_btn(
                "chk_transfer_truncate",
                cur_truncate,
                t("transfer_modal.opt_truncate_target", lang),
                on_truncate,
            ))
            .child(render_checkbox_btn(
                "chk_transfer_fk",
                cur_fk,
                t("transfer_modal.opt_disable_fk", lang),
                on_fk,
            ))
            .child(render_checkbox_btn(
                "chk_transfer_tx",
                cur_tx,
                t("transfer_modal.opt_transaction", lang),
                on_tx,
            ))
            .child(render_checkbox_btn(
                "chk_transfer_cont",
                cur_cont,
                t("transfer_modal.opt_continue_error", lang),
                on_cont,
            ))
            .child(
                v_flex()
                    .w_full()
                    .p_3()
                    .gap_2()
                    .rounded_md()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("transfer_modal.opt_batch_size", lang)),
                    )
                    .child(batch_btns),
            )
    }

    fn render_execution_step(&self, lang: AppLanguage) -> impl IntoElement {
        let mut content = v_flex().size_full().p_4().gap_3();

        // 1. Progress Bar Card
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
                                            "Table {}/{} ({:.1}%) - {}",
                                            prog.current_table_idx + 1,
                                            prog.total_tables,
                                            prog.percentage,
                                            prog.current_table_name
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!("{} rows", prog.total_rows_transferred)),
                        ),
                )
                .child(
                    // Visual progress bar
                    div()
                        .w_full()
                        .h(px(6.0))
                        .rounded_full()
                        .bg(ThemeColors::BG_APP)
                        .child(
                            div()
                                .h_full()
                                .w(px((prog.percentage * 6.5).clamp(4.0, 650.0)))
                                .rounded_full()
                                .bg(ThemeColors::PRIMARY_LIGHT),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(prog.message.clone()),
                );

            content = content.child(prog_card);
        }

        // 2. Summary Card
        if let Some(ref sum) = self.summary {
            let status_c = if sum.is_success {
                ThemeColors::SUCCESS
            } else {
                ThemeColors::ERROR
            };
            let status_title = if sum.aborted_early {
                t("transfer_modal.aborted", lang)
            } else {
                t("transfer_modal.completed", lang)
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
                            Icon::new(if sum.is_success {
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
                    h_flex()
                        .gap_4()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!(
                                    "{}: {}/{}",
                                    t("transfer_modal.summary_tables", lang),
                                    sum.tables_completed,
                                    sum.tables_total
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!(
                                    "{}: {}",
                                    t("transfer_modal.summary_rows", lang),
                                    sum.rows_transferred
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!(
                                    "{}: {}",
                                    t("transfer_modal.summary_errors", lang),
                                    sum.errors_count
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!(
                                    "{}: {:.2}s",
                                    t("transfer_modal.summary_time", lang),
                                    sum.duration_ms as f64 / 1000.0
                                )),
                        ),
                );

            content = content.child(sum_card);
        }

        // 3. Scrollable Live Audit Log
        let mut log_list = v_flex().w_full().gap_1();
        for log in &self.logs {
            let color = match log.level {
                TransferLogLevel::Info => ThemeColors::TEXT_MUTED,
                TransferLogLevel::Ddl => ThemeColors::PRIMARY_LIGHT,
                TransferLogLevel::Data => ThemeColors::TEXT_PRIMARY,
                TransferLogLevel::Success => ThemeColors::SUCCESS,
                TransferLogLevel::Warn => ThemeColors::WARNING,
                TransferLogLevel::Error => ThemeColors::ERROR,
            };

            let prefix = match log.level {
                TransferLogLevel::Info => "[INFO]",
                TransferLogLevel::Ddl => "[DDL]",
                TransferLogLevel::Data => "[DATA]",
                TransferLogLevel::Success => "[OK]",
                TransferLogLevel::Warn => "[WARN]",
                TransferLogLevel::Error => "[ERR]",
            };

            log_list = log_list.child(
                h_flex()
                    .gap_2()
                    .text_size(px(11.0))
                    .items_start()
                    .child(
                        div()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(log.timestamp.clone()),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(color)
                            .child(prefix),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(color)
                            .child(log.message.clone()),
                    ),
            );
        }

        let log_box = v_flex()
            .id("transfer_audit_log_scroll")
            .flex_1()
            .p_3()
            .gap_1()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .overflow_y_scroll()
            .child(log_list);

        content = content.child(log_box);

        content
    }

    fn render_footer(&self, lang: AppLanguage) -> impl IntoElement {
        let on_close = self.on_close.clone();
        let on_start = self.on_start_transfer.clone();
        let on_abort = self.on_abort_transfer.clone();
        let on_copy = self.on_copy_logs.clone();
        let is_running = self.is_running;
        let is_exec_step = self.step == TransferModalStep::Execution;
        let has_summary = self.summary.is_some();
        let copied = self.copied;

        let selected_tables_count = self.tables.iter().filter(|t| t.selected).count();
        let can_start = !is_running
            && selected_tables_count > 0
            && self.source_conn_id.is_some()
            && self.target_conn_id.is_some()
            && self.source_db.is_some()
            && self.target_db.is_some()
            && !(self.source_conn_id == self.target_conn_id && self.source_db == self.target_db);

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
                // Left side: Step info or copy log
                h_flex()
                    .items_center()
                    .gap_2()
                    .when(is_exec_step && (is_running || has_summary), |this| {
                        this.child(
                            Button::new("transfer_copy_log_btn")
                                .small()
                                .outline()
                                .icon(if copied {
                                    IconName::Check
                                } else {
                                    IconName::Copy
                                })
                                .label(if copied {
                                    t("transfer_modal.logs_copied", lang)
                                } else {
                                    t("transfer_modal.copy_logs", lang)
                                })
                                .on_click(move |_, window, cx| {
                                    if let Some(ref h) = on_copy {
                                        h(window, cx);
                                    }
                                }),
                        )
                    }),
            )
            .child(
                // Right side: navigation / action buttons
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("transfer_close_btn")
                            .small()
                            .ghost()
                            .label(if has_summary { "Done" } else { "Cancel" })
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_close {
                                    h(window, cx);
                                }
                            }),
                    )
                    .when(is_running, |this| {
                        this.child(
                            Button::new("transfer_abort_btn")
                                .small()
                                .danger()
                                .icon(IconName::Close)
                                .label(t("transfer_modal.abort_transfer", lang))
                                .on_click(move |_, window, cx| {
                                    if let Some(ref h) = on_abort {
                                        h(window, cx);
                                    }
                                }),
                        )
                    })
                    .when(!is_running && self.step != TransferModalStep::Execution, |this| {
                        let mut btn = Button::new("transfer_start_btn")
                            .small()
                            .primary()
                            .icon(IconName::Play)
                            .label(t("transfer_modal.btn_transfer", lang));
                        if !can_start {
                            btn = btn.disabled(true);
                        }
                        this.child(
                            btn.on_click(move |_, window, cx| {
                                if let Some(ref h) = on_start {
                                    h(window, cx);
                                }
                            }),
                        )
                    }),
            )
    }
}

impl RenderOnce for TransferModal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let lang = self.language;
        let on_close = self.on_close.clone();

        let header = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(
                h_flex()
                    .items_center()
                    .gap_2p5()
                    .child(
                        Icon::new(IconName::Database)
                            .size(px(20.0))
                            .text_color(ThemeColors::PRIMARY_LIGHT),
                    )
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("transfer_modal.title", lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("transfer_modal.subtitle", lang)),
                            ),
                    ),
            )
            .child(
                Button::new("transfer_x_close")
                    .ghost()
                    .small()
                    .icon(IconName::Close)
                    .on_click(move |_, window, cx| {
                        if let Some(ref h) = on_close {
                            h(window, cx);
                        }
                    }),
            );

        let step_nav = h_flex()
            .w_full()
            .px_4()
            .py_2()
            .gap_2()
            .bg(ThemeColors::BG_SURFACE)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(self.render_step_tab(
                TransferModalStep::SourceTarget,
                t("transfer_modal.step_source_target", lang),
                IconName::Database,
            ))
            .child(self.render_step_tab(
                TransferModalStep::TableMapping,
                t("transfer_modal.step_tables", lang),
                IconName::Table,
            ))
            .child(self.render_step_tab(
                TransferModalStep::Options,
                t("transfer_modal.step_options", lang),
                IconName::Settings,
            ))
            .child(self.render_step_tab(
                TransferModalStep::Execution,
                t("transfer_modal.step_execution", lang),
                IconName::Terminal,
            ));

        let body: AnyElement = match self.step {
            TransferModalStep::SourceTarget => self.render_source_target_step(lang).into_any_element(),
            TransferModalStep::TableMapping => self.render_table_mapping_step(lang).into_any_element(),
            TransferModalStep::Options => self.render_options_step(lang).into_any_element(),
            TransferModalStep::Execution => self.render_execution_step(lang).into_any_element(),
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
                    .w(px(740.0))
                    .max_h(px(620.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_modal_initialization() {
        let modal = TransferModal::new(
            TransferModalStep::SourceTarget,
            Some("conn_1".to_string()),
            "Prod MySQL".to_string(),
            Some("ecommerce".to_string()),
            Some("conn_2".to_string()),
            "Dev Postgres".to_string(),
            Some("ecommerce_dev".to_string()),
            vec![
                ("conn_1".to_string(), "Prod MySQL".to_string()),
                ("conn_2".to_string(), "Dev Postgres".to_string()),
            ],
            vec!["ecommerce".to_string(), "analytics".to_string()],
            vec!["ecommerce_dev".to_string()],
            vec![
                TransferTableMapping::new("users"),
                TransferTableMapping::new("orders"),
            ],
            String::new(),
            TransferOptions::default(),
            false,
            None,
            None,
            Vec::new(),
            None,
            false,
            AppLanguage::En,
        );

        assert_eq!(modal.step, TransferModalStep::SourceTarget);
        assert_eq!(modal.source_conn_name, "Prod MySQL");
        assert_eq!(modal.target_conn_name, "Dev Postgres");
        assert_eq!(modal.tables.len(), 2);
        assert!(!modal.is_running);
    }
}
