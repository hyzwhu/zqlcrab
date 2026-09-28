//! Visual Database & Multi-Table Dump Wizard Modal Dialog.
//! Allows selecting tables, configuring schema/data scopes, safety options, and exporting to SQL scripts, clipboard, or console.

use crate::db::dump::{DatabaseDumpConfig, DumpDestination, DumpProgress, DumpSummary};
use crate::db::export::ExportScope;
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
    spinner::Spinner,
};
use gpui_kit::gpui::{
    AnyElement, App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, px, rgba,
};
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DumpModalStep {
    #[default]
    SelectTables,
    Options,
    Destination,
}

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct DumpModal {
    step: DumpModalStep,
    config: DatabaseDumpConfig,
    available_tables: Vec<String>,
    selected_tables: HashSet<String>,
    filter_query: String,
    file_path: Option<String>,
    is_exporting: bool,
    progress: Option<DumpProgress>,
    summary: Option<DumpSummary>,
    error_msg: Option<String>,
    copied: bool,
    language: AppLanguage,

    on_step_change: Option<Rc<dyn Fn(DumpModalStep, &mut Window, &mut App) + 'static>>,
    on_toggle_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_all: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_deselect_all: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_change_scope: Option<Rc<dyn Fn(ExportScope, &mut Window, &mut App) + 'static>>,
    on_toggle_drop_table: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_transaction: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_fk_checks: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_comments: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_change_batch_size: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_change_destination: Option<Rc<dyn Fn(DumpDestination, &mut Window, &mut App) + 'static>>,
    on_browse_file: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_start_dump: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_open_in_console: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl DumpModal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        step: DumpModalStep,
        config: DatabaseDumpConfig,
        available_tables: Vec<String>,
        selected_tables: HashSet<String>,
        filter_query: String,
        file_path: Option<String>,
        is_exporting: bool,
        progress: Option<DumpProgress>,
        summary: Option<DumpSummary>,
        error_msg: Option<String>,
        copied: bool,
        language: AppLanguage,
    ) -> Self {
        Self {
            step,
            config,
            available_tables,
            selected_tables,
            filter_query,
            file_path,
            is_exporting,
            progress,
            summary,
            error_msg,
            copied,
            language,
            on_step_change: None,
            on_toggle_table: None,
            on_select_all: None,
            on_deselect_all: None,
            on_change_scope: None,
            on_toggle_drop_table: None,
            on_toggle_transaction: None,
            on_toggle_fk_checks: None,
            on_toggle_comments: None,
            on_change_batch_size: None,
            on_change_destination: None,
            on_browse_file: None,
            on_start_dump: None,
            on_open_in_console: None,
            on_close: None,
        }
    }

    pub fn on_step_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(DumpModalStep, &mut Window, &mut App) + 'static,
    {
        self.on_step_change = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_table = Some(Rc::new(handler));
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

    pub fn on_change_scope<F>(mut self, handler: F) -> Self
    where
        F: Fn(ExportScope, &mut Window, &mut App) + 'static,
    {
        self.on_change_scope = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_drop_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_drop_table = Some(Rc::new(handler));
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

    pub fn on_toggle_comments<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_comments = Some(Rc::new(handler));
        self
    }

    pub fn on_change_batch_size<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_change_batch_size = Some(Rc::new(handler));
        self
    }

    pub fn on_change_destination<F>(mut self, handler: F) -> Self
    where
        F: Fn(DumpDestination, &mut Window, &mut App) + 'static,
    {
        self.on_change_destination = Some(Rc::new(handler));
        self
    }

    pub fn on_browse_file<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_browse_file = Some(Rc::new(handler));
        self
    }

    pub fn on_start_dump<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_start_dump = Some(Rc::new(handler));
        self
    }

    pub fn on_open_in_console<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_open_in_console = Some(Rc::new(handler));
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
        step: DumpModalStep,
        label: &'static str,
        icon: IconName,
    ) -> impl IntoElement {
        let is_active = self.step == step;
        let handler = self.on_step_change.clone();

        let mut btn = Button::new(match step {
            DumpModalStep::SelectTables => "dump_step_tab_select",
            DumpModalStep::Options => "dump_step_tab_options",
            DumpModalStep::Destination => "dump_step_tab_dest",
        })
        .small()
        .icon(icon)
        .label(label);

        if is_active {
            btn = btn.primary();
        } else {
            btn = btn.ghost();
        }

        if !self.is_exporting
            && let Some(h) = handler
        {
            btn = btn.on_click(move |_, window, cx| {
                h(step, window, cx);
            });
        }

        btn
    }
}

impl RenderOnce for DumpModal {
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
                                Icon::new(IconName::Database)
                                    .size(px(18.0))
                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("dump_modal.title", lang)),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::BG_SURFACE_HOVER)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .text_size(px(11.0))
                                    .child(format!("{:?}", self.config.family)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("dump_modal.subtitle", lang)),
                    ),
            )
            .child(
                Button::new("dump_modal_close_btn")
                    .ghost()
                    .small()
                    .icon(IconName::X)
                    .on_click(move |_, window, cx| {
                        if let Some(ref h) = on_close_click {
                            h(window, cx);
                        }
                    }),
            );

        // Wizard Step Navigation Bar
        let step_nav = h_flex()
            .w_full()
            .px_4()
            .py_2()
            .gap_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(self.render_step_tab(
                DumpModalStep::SelectTables,
                t("dump_modal.step_select", lang),
                IconName::Table,
            ))
            .child(self.render_step_tab(
                DumpModalStep::Options,
                t("dump_modal.step_options", lang),
                IconName::Settings,
            ))
            .child(self.render_step_tab(
                DumpModalStep::Destination,
                t("dump_modal.step_destination", lang),
                IconName::Download,
            ));

        // Step Content Body
        let body: AnyElement = match self.step {
            DumpModalStep::SelectTables => self.render_select_tables_step(lang).into_any_element(),
            DumpModalStep::Options => self.render_options_step(lang).into_any_element(),
            DumpModalStep::Destination => self.render_destination_step(lang).into_any_element(),
        };

        // Footer Actions
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

impl DumpModal {
    fn render_select_tables_step(&self, lang: AppLanguage) -> impl IntoElement {
        let on_sel_all = self.on_select_all.clone();
        let on_desel_all = self.on_deselect_all.clone();
        let selected_count = self.selected_tables.len();
        let total_count = self.available_tables.len();

        let toolbar = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_4()
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
                    .gap_2()
                    .child(
                        Button::new("dump_sel_all_btn")
                            .outline()
                            .xsmall()
                            .label(t("dump_modal.select_all", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_sel_all {
                                    h(window, cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("dump_desel_all_btn")
                            .outline()
                            .xsmall()
                            .label(t("dump_modal.deselect_all", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_desel_all {
                                    h(window, cx);
                                }
                            }),
                    ),
            );

        let mut list = v_flex().w_full().gap_1().p_2();

        let query = self.filter_query.to_lowercase();
        let filtered: Vec<&String> = self
            .available_tables
            .iter()
            .filter(|tbl| query.is_empty() || tbl.to_lowercase().contains(&query))
            .collect();

        if filtered.is_empty() {
            list = list.child(
                div()
                    .p_6()
                    .text_center()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child("No tables found"),
            );
        } else {
            for tbl in filtered {
                let is_checked = self.selected_tables.contains(tbl);
                let tbl_name = tbl.clone();
                let toggle_handler = self.on_toggle_table.clone();

                let check_icon = if is_checked {
                    IconName::Check
                } else {
                    IconName::Circle
                };

                let mut item_btn = Button::new(format!("tbl_item_{tbl_name}"))
                    .small()
                    .w_full()
                    .icon(check_icon)
                    .label(tbl_name.clone());

                if is_checked {
                    item_btn = item_btn.primary();
                } else {
                    item_btn = item_btn.outline();
                }

                if let Some(h) = toggle_handler {
                    let name = tbl_name.clone();
                    item_btn = item_btn.on_click(move |_, window, cx| {
                        h(name.clone(), window, cx);
                    });
                }

                list = list.child(item_btn);
            }
        }

        v_flex()
            .size_full()
            .child(toolbar)
            .child(
                v_flex()
                    .id("dump_tables_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list),
            )
    }

    #[allow(clippy::type_complexity)]
    fn render_options_step(&self, lang: AppLanguage) -> impl IntoElement {
        let cur_scope = self.config.scope;
        let on_scope = self.on_change_scope.clone();

        let render_scope_btn = |scope: ExportScope, label: &'static str, icon: IconName| {
            let is_sel = cur_scope == scope;
            let on_scope_click = on_scope.clone();

            let mut btn = Button::new(match scope {
                ExportScope::SchemaAndData => "scope_btn_full",
                ExportScope::SchemaOnly => "scope_btn_schema",
                ExportScope::DataOnly => "scope_btn_data",
            })
            .small()
            .flex_1()
            .icon(icon)
            .label(label);

            if is_sel {
                btn = btn.primary();
            } else {
                btn = btn.outline();
            }

            if let Some(h) = on_scope_click {
                btn = btn.on_click(move |_, window, cx| {
                    h(scope, window, cx);
                });
            }
            btn
        };

        let scope_row = h_flex()
            .w_full()
            .gap_3()
            .child(render_scope_btn(
                ExportScope::SchemaAndData,
                t("dump_modal.scope_full", lang),
                IconName::Database,
            ))
            .child(render_scope_btn(
                ExportScope::SchemaOnly,
                t("dump_modal.scope_schema", lang),
                IconName::Table,
            ))
            .child(render_scope_btn(
                ExportScope::DataOnly,
                t("dump_modal.scope_data", lang),
                IconName::FileText,
            ));

        // Switches & Checkboxes
        let on_drop = self.on_toggle_drop_table.clone();
        let cur_drop = self.config.options.drop_table_if_exists;
        let on_tx = self.on_toggle_transaction.clone();
        let cur_tx = self.config.options.wrap_in_transaction;
        let on_fk = self.on_toggle_fk_checks.clone();
        let cur_fk = self.config.disable_foreign_keys;
        let on_comments = self.on_toggle_comments.clone();
        let cur_comments = self.config.options.include_comments;

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

        let cur_batch = self.config.options.batch_size;
        let on_batch = self.on_change_batch_size.clone();

        let batch_menu = Button::new("dump_batch_btn")
            .outline()
            .small()
            .label(format!("{cur_batch} rows"))
            .dropdown_menu(move |mut menu, _window, _cx| {
                let b_handler = on_batch.clone();
                menu = menu.item(
                    PopupMenuItem::new("100 rows per INSERT")
                        .on_click({
                            let h = b_handler.clone();
                            move |_, window, cx| {
                                if let Some(ref h) = h {
                                    h(100, window, cx);
                                }
                            }
                        }),
                );
                menu = menu.item(
                    PopupMenuItem::new("200 rows per INSERT")
                        .on_click({
                            let h = b_handler.clone();
                            move |_, window, cx| {
                                if let Some(ref h) = h {
                                    h(200, window, cx);
                                }
                            }
                        }),
                );
                menu = menu.item(
                    PopupMenuItem::new("500 rows per INSERT")
                        .on_click({
                            let h = b_handler.clone();
                            move |_, window, cx| {
                                if let Some(ref h) = h {
                                    h(500, window, cx);
                                }
                            }
                        }),
                );
                menu = menu.item(
                    PopupMenuItem::new("1000 rows per INSERT")
                        .on_click({
                            let h = b_handler;
                            move |_, window, cx| {
                                if let Some(ref h) = h {
                                    h(1000, window, cx);
                                }
                            }
                        }),
                );
                menu
            });

        let batch_row = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .child(
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(t("dump_modal.opt_batch_size", lang)),
            )
            .child(batch_menu);

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(scope_row)
            .child(render_checkbox_btn(
                "chk_drop_table",
                cur_drop,
                t("dump_modal.opt_drop_table", lang),
                on_drop,
            ))
            .child(render_checkbox_btn(
                "chk_transaction",
                cur_tx,
                t("dump_modal.opt_transaction", lang),
                on_tx,
            ))
            .child(render_checkbox_btn(
                "chk_fk_checks",
                cur_fk,
                t("dump_modal.opt_fk_checks", lang),
                on_fk,
            ))
            .child(render_checkbox_btn(
                "chk_comments",
                cur_comments,
                t("dump_modal.opt_comments", lang),
                on_comments,
            ))
            .child(batch_row)
    }

    fn render_destination_step(&self, lang: AppLanguage) -> impl IntoElement {
        let cur_dest = self.config.destination;
        let on_dest = self.on_change_destination.clone();

        let render_dest_btn = |dest: DumpDestination, label: &'static str, icon: IconName| {
            let is_sel = cur_dest == dest;
            let on_dest_click = on_dest.clone();

            let mut btn = Button::new(match dest {
                DumpDestination::File => "dest_btn_file",
                DumpDestination::Clipboard => "dest_btn_clip",
                DumpDestination::QueryConsole => "dest_btn_console",
            })
            .small()
            .w_full()
            .icon(icon)
            .label(label);

            if is_sel {
                btn = btn.primary();
            } else {
                btn = btn.outline();
            }

            if let Some(h) = on_dest_click {
                btn = btn.on_click(move |_, window, cx| {
                    h(dest, window, cx);
                });
            }
            btn
        };

        let on_browse = self.on_browse_file.clone();
        let file_row = if cur_dest == DumpDestination::File {
            let path_display = self
                .file_path
                .clone()
                .unwrap_or_else(|| "No file path selected (Click Browse)".to_string());
            Some(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(path_display),
                    )
                    .child(
                        Button::new("dump_browse_btn")
                            .outline()
                            .small()
                            .icon(IconName::Folder)
                            .label("Browse...")
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_browse {
                                    h(window, cx);
                                }
                            }),
                    ),
            )
        } else {
            None
        };

        let mut content = v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(render_dest_btn(
                DumpDestination::File,
                t("dump_modal.dest_file", lang),
                IconName::Folder,
            ))
            .child(render_dest_btn(
                DumpDestination::Clipboard,
                if self.copied {
                    "✓ Copied to Clipboard!"
                } else {
                    t("dump_modal.dest_clipboard", lang)
                },
                IconName::Copy,
            ))
            .child(render_dest_btn(
                DumpDestination::QueryConsole,
                t("dump_modal.dest_console", lang),
                IconName::Terminal,
            ));

        if let Some(fr) = file_row {
            content = content.child(fr);
        }

        // Live Progress or Summary Status Card
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
                                            "Table {} / {}: {}",
                                            prog.current_table_index,
                                            prog.total_tables,
                                            prog.current_table_name
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(format!("{} rows", prog.total_rows_exported)),
                        ),
                );
            content = content.child(prog_card);
        } else if let Some(ref sum) = self.summary {
            let sum_card = v_flex()
                .w_full()
                .p_3()
                .gap_1p5()
                .rounded_md()
                .bg(rgba(0x28a74515))
                .border_1()
                .border_color(ThemeColors::SUCCESS)
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::Check)
                                .size(px(16.0))
                                .text_color(ThemeColors::SUCCESS),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::SUCCESS)
                                .child(t("dump_modal.completed", lang)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(format!(
                            "{} tables | {} rows | {} bytes | {} ms",
                            sum.tables_count, sum.total_rows, sum.total_bytes, sum.duration_ms
                        )),
                );
            content = content.child(sum_card);
        }

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
        let on_start = self.on_start_dump.clone();
        let is_running = self.is_exporting;
        let selected_empty = self.selected_tables.is_empty();

        let mut dump_btn = Button::new("dump_start_btn")
            .primary()
            .small()
            .icon(IconName::Download)
            .label(if is_running {
                t("dump_modal.dumping", lang)
            } else {
                t("dump_modal.btn_dump", lang)
            });

        if is_running || selected_empty {
            // Disabled when running or no tables selected
        } else if let Some(ref h) = on_start {
            let h = h.clone();
            dump_btn = dump_btn.on_click(move |_, window, cx| {
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
                    .child(if selected_empty {
                        "Please select at least 1 table to dump"
                    } else {
                        ""
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("dump_cancel_btn")
                            .outline()
                            .small()
                            .label(t("common.close", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = on_close_click {
                                    h(window, cx);
                                }
                            }),
                    )
                    .child(dump_btn),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dump_modal_initialization() {
        let mut selected = HashSet::new();
        selected.insert("users".to_string());

        let modal = DumpModal::new(
            DumpModalStep::SelectTables,
            DatabaseDumpConfig::default(),
            vec!["users".to_string(), "posts".to_string()],
            selected,
            String::new(),
            None,
            false,
            None,
            None,
            None,
            false,
            AppLanguage::En,
        );

        assert_eq!(modal.step, DumpModalStep::SelectTables);
        assert_eq!(modal.available_tables.len(), 2);
        assert!(modal.selected_tables.contains("users"));
    }
}
