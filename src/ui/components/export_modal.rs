//! Visual Data & Schema Export Wizard & Table Dump Modal.
//! Supports exporting entire tables (DDL + Data) or subsets into SQL Dump, CSV, TSV, JSON, NDJSON, and Markdown.

use crate::db::export::{ExportFormat, ExportScope, TableDumpConfig};
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
    menu::{DropdownMenu as _, PopupMenuItem},
    spinner::Spinner,
};
use gpui_kit::gpui::{
    Anchor, App, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window, div,
    prelude::FluentBuilder as _, px, rgba,
};
use std::rc::Rc;

/// Steps of the Export Wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportWizardStep {
    #[default]
    Step1Config,
    Step2Preview,
    Step3Progress,
}

/// Output destination choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportDestination {
    #[default]
    File,
    Clipboard,
}

/// Successful export result summary.
#[derive(Debug, Clone)]
pub struct ExportSuccessInfo {
    pub rows_count: usize,
    pub bytes_written: usize,
    pub elapsed_millis: u64,
    pub file_path: Option<String>,
    pub copied_to_clipboard: bool,
}

/// Export Wizard Modal Component.
#[derive(IntoElement)]
pub struct ExportModal {
    step: ExportWizardStep,
    target_table: String,
    available_tables: Vec<String>,
    family: DatabaseFamily,
    config: TableDumpConfig,
    destination: ExportDestination,
    file_path_input: Entity<InputState>,
    where_input: Entity<InputState>,
    limit_input: Entity<InputState>,
    preview_content: Option<String>,
    is_loading_preview: bool,
    is_executing: bool,
    progress_rows: usize,
    success_info: Option<ExportSuccessInfo>,
    error_msg: Option<String>,
    lang: AppLanguage,

    // Callbacks
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_select_step: Option<Rc<dyn Fn(ExportWizardStep, &mut Window, &mut App) + 'static>>,
    on_select_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_format: Option<Rc<dyn Fn(ExportFormat, &mut Window, &mut App) + 'static>>,
    on_select_scope: Option<Rc<dyn Fn(ExportScope, &mut Window, &mut App) + 'static>>,
    on_select_destination: Option<Rc<dyn Fn(ExportDestination, &mut Window, &mut App) + 'static>>,
    on_browse_file: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_toggle_drop_table: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_transaction: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_select_batch_size: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_toggle_headers: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_select_delimiter: Option<Rc<dyn Fn(char, &mut Window, &mut App) + 'static>>,
    on_select_null_rep: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_toggle_pretty_json: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_refresh_preview: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_start_export: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_reveal_file: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
}

impl ExportModal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        step: ExportWizardStep,
        target_table: String,
        available_tables: Vec<String>,
        family: DatabaseFamily,
        config: TableDumpConfig,
        destination: ExportDestination,
        file_path_input: &Entity<InputState>,
        where_input: &Entity<InputState>,
        limit_input: &Entity<InputState>,
        is_executing: bool,
        lang: AppLanguage,
    ) -> Self {
        Self {
            step,
            target_table,
            available_tables,
            family,
            config,
            destination,
            file_path_input: file_path_input.clone(),
            where_input: where_input.clone(),
            limit_input: limit_input.clone(),
            preview_content: None,
            is_loading_preview: false,
            is_executing,
            progress_rows: 0,
            success_info: None,
            error_msg: None,
            lang,
            on_close: None,
            on_select_step: None,
            on_select_table: None,
            on_select_format: None,
            on_select_scope: None,
            on_select_destination: None,
            on_browse_file: None,
            on_toggle_drop_table: None,
            on_toggle_transaction: None,
            on_select_batch_size: None,
            on_toggle_headers: None,
            on_select_delimiter: None,
            on_select_null_rep: None,
            on_toggle_pretty_json: None,
            on_refresh_preview: None,
            on_start_export: None,
            on_reveal_file: None,
        }
    }

    pub fn preview(mut self, content: Option<String>, is_loading: bool) -> Self {
        self.preview_content = content;
        self.is_loading_preview = is_loading;
        self
    }

    pub fn progress(mut self, rows: usize) -> Self {
        self.progress_rows = rows;
        self
    }

    pub fn success(mut self, info: Option<ExportSuccessInfo>) -> Self {
        self.success_info = info;
        self
    }

    pub fn error(mut self, err: Option<String>) -> Self {
        self.error_msg = err;
        self
    }

    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    pub fn on_select_step<F>(mut self, handler: F) -> Self
    where
        F: Fn(ExportWizardStep, &mut Window, &mut App) + 'static,
    {
        self.on_select_step = Some(Rc::new(handler));
        self
    }

    pub fn on_select_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_table = Some(Rc::new(handler));
        self
    }

    pub fn on_select_format<F>(mut self, handler: F) -> Self
    where
        F: Fn(ExportFormat, &mut Window, &mut App) + 'static,
    {
        self.on_select_format = Some(Rc::new(handler));
        self
    }

    pub fn on_select_scope<F>(mut self, handler: F) -> Self
    where
        F: Fn(ExportScope, &mut Window, &mut App) + 'static,
    {
        self.on_select_scope = Some(Rc::new(handler));
        self
    }

    pub fn on_select_destination<F>(mut self, handler: F) -> Self
    where
        F: Fn(ExportDestination, &mut Window, &mut App) + 'static,
    {
        self.on_select_destination = Some(Rc::new(handler));
        self
    }

    pub fn on_browse_file<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_browse_file = Some(Rc::new(handler));
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

    pub fn on_select_batch_size<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_select_batch_size = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_headers<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_headers = Some(Rc::new(handler));
        self
    }

    pub fn on_select_delimiter<F>(mut self, handler: F) -> Self
    where
        F: Fn(char, &mut Window, &mut App) + 'static,
    {
        self.on_select_delimiter = Some(Rc::new(handler));
        self
    }

    pub fn on_select_null_rep<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_null_rep = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_pretty_json<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_pretty_json = Some(Rc::new(handler));
        self
    }

    pub fn on_refresh_preview<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh_preview = Some(Rc::new(handler));
        self
    }

    pub fn on_start_export<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_start_export = Some(Rc::new(handler));
        self
    }

    pub fn on_reveal_file<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_reveal_file = Some(Rc::new(handler));
        self
    }

    fn render_step_indicator(&self) -> impl IntoElement {
        let steps = [
            ExportWizardStep::Step1Config,
            ExportWizardStep::Step2Preview,
            ExportWizardStep::Step3Progress,
        ];

        let on_step = self.on_select_step.clone();
        let current_step = self.step;
        let is_executing = self.is_executing;
        let lang = self.lang;

        h_flex()
            .gap_1()
            .items_center()
            .children(steps.iter().enumerate().map(|(idx, step_item)| {
                let is_active = *step_item == current_step;
                let is_past = (*step_item as usize) < (current_step as usize);
                let label = match step_item {
                    ExportWizardStep::Step1Config => t("export_modal.step_config", lang),
                    ExportWizardStep::Step2Preview => t("export_modal.step_preview", lang),
                    ExportWizardStep::Step3Progress => t("export_modal.step_execute", lang),
                };

                let step_val = *step_item;
                let on_step_cloned = on_step.clone();

                let mut btn = div()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .font_weight(if is_active {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .cursor_pointer();

                if is_active {
                    btn = btn
                        .bg(ThemeColors::PRIMARY_BORDER)
                        .text_color(ThemeColors::TEXT_PRIMARY);
                } else if is_past {
                    btn = btn
                        .bg(ThemeColors::BG_SURFACE_HOVER)
                        .text_color(ThemeColors::SUCCESS);
                } else {
                    btn = btn
                        .bg(ThemeColors::BG_SURFACE)
                        .text_color(ThemeColors::TEXT_MUTED);
                }

                if !is_executing {
                    if let Some(ref handler) = on_step_cloned {
                        let handler = handler.clone();
                        btn = btn.on_mouse_down(
                            gpui_kit::gpui::MouseButton::Left,
                            move |_, window, cx| {
                                handler(step_val, window, cx);
                            },
                        );
                    }
                }

                h_flex()
                    .items_center()
                    .gap_1()
                    .child(btn.child(format!("{}. {}", idx + 1, label)))
                    .when(idx < steps.len() - 1, |this| {
                        this.child(
                            Icon::new(IconName::ChevronRight)
                                .size(px(12.0))
                                .text_color(ThemeColors::TEXT_MUTED),
                        )
                    })
            }))
    }

    fn render_step1_config(&self) -> impl IntoElement {
        let lang = self.lang;
        let on_sel_table = self.on_select_table.clone();
        let on_sel_fmt = self.on_select_format.clone();
        let on_sel_scope = self.on_select_scope.clone();
        let on_sel_dest = self.on_select_destination.clone();
        let on_browse = self.on_browse_file.clone();
        let on_step = self.on_select_step.clone();
        let on_start = self.on_start_export.clone();

        // 1. Table Selector Dropdown
        let cur_tbl = self.target_table.clone();
        let tables_clone = self.available_tables.clone();
        let tbl_label = if cur_tbl.is_empty() {
            t("export_modal.select_table", lang).to_string()
        } else {
            cur_tbl.clone()
        };
        let table_menu_btn = Button::new("export_target_table_dropdown")
            .outline()
            .small()
            .icon(IconName::Table)
            .label(tbl_label)
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for tbl in &tables_clone {
                    let tbl_name = tbl.clone();
                    let sel = on_sel_table.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tbl.clone())
                            .icon(IconName::Table)
                            .on_click(move |_, window, cx| {
                                if let Some(ref h) = sel {
                                    h(tbl_name.clone(), window, cx);
                                }
                            }),
                    );
                }
                menu
            });

        // 2. Format Buttons
        let formats = [
            (ExportFormat::SqlDump, IconName::Database, "SQL Dump"),
            (ExportFormat::Csv, IconName::FileText, "CSV"),
            (ExportFormat::Json, IconName::Code, "JSON"),
            (ExportFormat::Ndjson, IconName::FileCode, "NDJSON"),
            (ExportFormat::Tsv, IconName::FileSpreadsheet, "TSV"),
            (ExportFormat::Markdown, IconName::BookOpen, "Markdown"),
        ];

        let cur_fmt = self.config.format;
        let format_buttons = h_flex()
            .gap_1p5()
            .items_center()
            .children(formats.iter().map(|(fmt, icon, label)| {
                let is_sel = *fmt == cur_fmt;
                let fmt_val = *fmt;
                let sel = on_sel_fmt.clone();

                let mut btn = Button::new(ElementId::Name(format!("fmt_{fmt:?}").into()))
                    .small()
                    .icon(*icon)
                    .label(*label);

                if is_sel {
                    btn = btn.primary();
                } else {
                    btn = btn.outline();
                }

                if let Some(ref handler) = sel {
                    let handler = handler.clone();
                    btn = btn.on_click(move |_, window, cx| {
                        handler(fmt_val, window, cx);
                    });
                }
                btn
            }));

        // 3. Scope Selector (only meaningful for SQL formats)
        let is_sql = matches!(
            self.config.format,
            ExportFormat::SqlDump | ExportFormat::SqlInsert
        );
        let cur_scope = self.config.scope;
        let scopes = [
            (
                ExportScope::SchemaAndData,
                t("export_modal.scope_all", lang),
            ),
            (ExportScope::DataOnly, t("export_modal.scope_data", lang)),
            (
                ExportScope::SchemaOnly,
                t("export_modal.scope_structure", lang),
            ),
        ];

        let scope_buttons = h_flex()
            .gap_1p5()
            .items_center()
            .children(scopes.iter().map(|(sc, label)| {
                let is_sel = *sc == cur_scope;
                let sc_val = *sc;
                let sel = on_sel_scope.clone();

                let mut btn = Button::new(ElementId::Name(format!("scope_{sc:?}").into()))
                    .small()
                    .label(*label);

                if is_sel {
                    btn = btn.primary();
                } else {
                    btn = btn.outline();
                }

                if is_sql {
                    if let Some(ref handler) = sel {
                        let handler = handler.clone();
                        btn = btn.on_click(move |_, window, cx| {
                            handler(sc_val, window, cx);
                        });
                    }
                } else {
                    // Non-SQL formats are always DataOnly
                    btn = btn.ghost().text_color(ThemeColors::TEXT_FAINT);
                }
                btn
            }));

        // 4. Destination Selector
        let cur_dest = self.destination;
        let dest_buttons = h_flex()
            .gap_1p5()
            .items_center()
            .child({
                let mut b = Button::new("dest_file")
                    .small()
                    .icon(IconName::HardDrive)
                    .label(t("export_modal.dest_file", lang));
                if cur_dest == ExportDestination::File {
                    b = b.primary();
                } else {
                    b = b.outline();
                }
                if let Some(ref h) = on_sel_dest {
                    let h = h.clone();
                    b = b.on_click(move |_, window, cx| {
                        h(ExportDestination::File, window, cx);
                    });
                }
                b
            })
            .child({
                let mut b = Button::new("dest_clip")
                    .small()
                    .icon(IconName::Copy)
                    .label(t("export_modal.dest_clipboard", lang));
                if cur_dest == ExportDestination::Clipboard {
                    b = b.primary();
                } else {
                    b = b.outline();
                }
                if let Some(ref h) = on_sel_dest {
                    let h = h.clone();
                    b = b.on_click(move |_, window, cx| {
                        h(ExportDestination::Clipboard, window, cx);
                    });
                }
                b
            });

        // 5. Format-specific options view
        let format_options_card = self.render_format_options_card();

        // Footer buttons
        let to_preview = on_step.clone();
        let preview_btn = Button::new("goto_preview_btn")
            .primary()
            .icon(IconName::Eye)
            .label(t("export_modal.preview_btn", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = to_preview {
                    h(ExportWizardStep::Step2Preview, window, cx);
                }
            });

        let direct_export_btn = Button::new("direct_export_btn")
            .outline()
            .icon(IconName::Download)
            .label(t("export_modal.export_now_btn", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_start {
                    h(window, cx);
                }
            });

        v_flex()
            .w_full()
            .gap_4()
            .p_6()
            // Form rows
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!(
                                        "{} ({:?})",
                                        t("export_modal.target_table", lang),
                                        self.family
                                    )),
                            )
                            .child(table_menu_btn),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("export_modal.format", lang)),
                            )
                            .child(format_buttons),
                    ),
            )
            // Scope
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("export_modal.scope", lang)),
                    )
                    .child(scope_buttons),
            )
            // Query filter inputs (WHERE and LIMIT)
            .child(
                h_flex()
                    .gap_3()
                    .w_full()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("export_modal.where_clause", lang)),
                            )
                            .child(
                                Input::new(&self.where_input).small().w_full(),
                            ),
                    )
                    .child(
                        v_flex()
                            .w(px(140.0))
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("export_modal.row_limit", lang)),
                            )
                            .child(Input::new(&self.limit_input).small().w_full()),
                    ),
            )
            // Format-specific settings
            .child(format_options_card)
            // Destination options
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("export_modal.destination", lang)),
                    )
                    .child(dest_buttons)
                    .when(cur_dest == ExportDestination::File, |this| {
                        let mut browse_btn = Button::new("browse_file_btn")
                            .outline()
                            .icon(IconName::FolderOpen)
                            .label(t("export_modal.browse", lang));
                        if let Some(ref h) = on_browse {
                            let h = h.clone();
                            browse_btn = browse_btn.on_click(move |_, window, cx| {
                                h(window, cx);
                            });
                        }

                        this.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .child(Input::new(&self.file_path_input)),
                                )
                                .child(browse_btn),
                        )
                    }),
            )
            // Bottom Action Bar
            .child(
                h_flex()
                    .justify_end()
                    .gap_3()
                    .pt_3()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .child(direct_export_btn)
                    .child(preview_btn),
            )
    }

    fn render_format_options_card(&self) -> impl IntoElement {
        let lang = self.lang;
        let bg = ThemeColors::BG_SURFACE;
        let border = ThemeColors::BORDER;

        match self.config.format {
            ExportFormat::SqlDump | ExportFormat::SqlInsert => {
                let on_drop = self.on_toggle_drop_table.clone();
                let on_tx = self.on_toggle_transaction.clone();
                let on_batch = self.on_select_batch_size.clone();

                let cur_drop = self.config.sql_options.drop_table_if_exists;
                let cur_tx = self.config.sql_options.wrap_in_transaction;
                let cur_batch = self.config.sql_options.batch_size;

                let batch_sizes = [50, 100, 250, 500, 1000];
                let batch_dropdown = Button::new("sql_batch_dropdown")
                    .outline()
                    .small()
                    .label(format!("{cur_batch} rows/INSERT"))
                    .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                        for sz in batch_sizes {
                            let sel = on_batch.clone();
                            menu = menu.item(
                                PopupMenuItem::new(format!("{sz} rows"))
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref h) = sel {
                                            h(sz, window, cx);
                                        }
                                    }),
                            );
                        }
                        menu
                    });

                let mut drop_btn = Button::new("toggle_drop_table")
                    .small()
                    .icon(if cur_drop {
                        IconName::CircleCheck
                    } else {
                        IconName::Circle
                    })
                    .label(t("export_modal.drop_table_toggle", lang));
                if cur_drop {
                    drop_btn = drop_btn.primary();
                } else {
                    drop_btn = drop_btn.outline();
                }
                if let Some(ref h) = on_drop {
                    let h = h.clone();
                    drop_btn = drop_btn.on_click(move |_, window, cx| {
                        h(!cur_drop, window, cx);
                    });
                }

                let mut tx_btn = Button::new("toggle_tx")
                    .small()
                    .icon(if cur_tx {
                        IconName::CircleCheck
                    } else {
                        IconName::Circle
                    })
                    .label(t("export_modal.tx_toggle", lang));
                if cur_tx {
                    tx_btn = tx_btn.primary();
                } else {
                    tx_btn = tx_btn.outline();
                }
                if let Some(ref h) = on_tx {
                    let h = h.clone();
                    tx_btn = tx_btn.on_click(move |_, window, cx| {
                        h(!cur_tx, window, cx);
                    });
                }

                v_flex()
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(bg)
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("export_modal.sql_options", lang)),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(drop_btn)
                            .child(tx_btn)
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(t("export_modal.batch_size", lang)),
                                    )
                                    .child(batch_dropdown),
                            ),
                    )
            }
            ExportFormat::Csv | ExportFormat::Tsv => {
                let on_hdr = self.on_toggle_headers.clone();
                let on_delim = self.on_select_delimiter.clone();
                let on_null = self.on_select_null_rep.clone();

                let cur_hdr = self.config.csv_options.include_headers;
                let cur_delim = self.config.csv_options.delimiter;
                let cur_null = self.config.csv_options.null_representation.clone();

                let mut hdr_btn = Button::new("toggle_headers")
                    .small()
                    .icon(if cur_hdr {
                        IconName::CircleCheck
                    } else {
                        IconName::Circle
                    })
                    .label(t("export_modal.headers_toggle", lang));
                if cur_hdr {
                    hdr_btn = hdr_btn.primary();
                } else {
                    hdr_btn = hdr_btn.outline();
                }
                if let Some(ref h) = on_hdr {
                    let h = h.clone();
                    hdr_btn = hdr_btn.on_click(move |_, window, cx| {
                        h(!cur_hdr, window, cx);
                    });
                }

                let delimiters = [
                    (',', "Comma (,)"),
                    (';', "Semicolon (;)"),
                    ('\t', "Tab (\\t)"),
                    ('|', "Pipe (|)"),
                ];
                let delim_dropdown = Button::new("csv_delim_dropdown")
                    .outline()
                    .small()
                    .label(match cur_delim {
                        ',' => "Comma (,)",
                        ';' => "Semicolon (;)",
                        '\t' => "Tab (\\t)",
                        '|' => "Pipe (|)",
                        _ => "Custom",
                    })
                    .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                        for (ch, label) in delimiters {
                            let sel = on_delim.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label.to_string()).on_click(
                                    move |_, window, cx| {
                                        if let Some(ref h) = sel {
                                            h(ch, window, cx);
                                        }
                                    },
                                ),
                            );
                        }
                        menu
                    });

                let null_options = ["", "NULL", "\\N"];
                let null_dropdown = Button::new("csv_null_dropdown")
                    .outline()
                    .small()
                    .label(if cur_null.is_empty() {
                        "Empty (\"\")"
                    } else {
                        &cur_null
                    })
                    .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                        for val in null_options {
                            let sel = on_null.clone();
                            let v_str = val.to_string();
                            menu = menu.item(
                                PopupMenuItem::new(if val.is_empty() {
                                    "Empty (\"\")".to_string()
                                } else {
                                    val.to_string()
                                })
                                .on_click(move |_, window, cx| {
                                    if let Some(ref h) = sel {
                                        h(v_str.clone(), window, cx);
                                    }
                                }),
                            );
                        }
                        menu
                    });

                v_flex()
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(bg)
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("export_modal.csv_options", lang)),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(hdr_btn)
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(t("export_modal.delimiter", lang)),
                                    )
                                    .child(delim_dropdown),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(t("export_modal.null_as", lang)),
                                    )
                                    .child(null_dropdown),
                            ),
                    )
            }
            ExportFormat::Json | ExportFormat::Ndjson => {
                let on_pretty = self.on_toggle_pretty_json.clone();
                let cur_pretty = self.config.json_options.pretty;

                let mut pretty_btn = Button::new("toggle_pretty_json")
                    .small()
                    .icon(if cur_pretty {
                        IconName::CircleCheck
                    } else {
                        IconName::Circle
                    })
                    .label(t("export_modal.pretty_json", lang));
                if cur_pretty {
                    pretty_btn = pretty_btn.primary();
                } else {
                    pretty_btn = pretty_btn.outline();
                }
                if let Some(ref h) = on_pretty {
                    let h = h.clone();
                    pretty_btn = pretty_btn.on_click(move |_, window, cx| {
                        h(!cur_pretty, window, cx);
                    });
                }

                v_flex()
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(bg)
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("export_modal.json_options", lang)),
                    )
                    .child(pretty_btn)
            }
            ExportFormat::Markdown => v_flex()
                .w_full()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(border)
                .bg(bg)
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(t("export_modal.markdown_hint", lang)),
                ),
        }
    }

    fn render_step2_preview(&self) -> impl IntoElement {
        let lang = self.lang;
        let on_step = self.on_select_step.clone();
        let on_start = self.on_start_export.clone();
        let on_refresh = self.on_refresh_preview.clone();

        let to_config = on_step.clone();
        let back_btn = Button::new("preview_back_btn")
            .outline()
            .icon(IconName::ArrowLeft)
            .label(t("export_modal.back_btn", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = to_config {
                    h(ExportWizardStep::Step1Config, window, cx);
                }
            });

        let mut refresh_btn = Button::new("preview_refresh_btn")
            .outline()
            .icon(IconName::RefreshCw)
            .label(t("export_modal.refresh_preview", lang));
        if let Some(ref h) = on_refresh {
            let h = h.clone();
            refresh_btn = refresh_btn.on_click(move |_, window, cx| {
                h(window, cx);
            });
        }

        let start_export_btn = Button::new("preview_start_export_btn")
            .primary()
            .icon(IconName::Download)
            .label(t("export_modal.btn_export", lang))
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_start {
                    h(window, cx);
                }
            });

        let preview_body = if self.is_loading_preview {
            v_flex()
                .w_full()
                .h(px(320.0))
                .items_center()
                .justify_center()
                .gap_2()
                .child(Spinner::new())
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(t("export_modal.generating_preview", lang)),
                )
                .into_any_element()
        } else if let Some(ref content) = self.preview_content {
            v_flex()
                .id("export_preview_scroll")
                .w_full()
                .h(px(320.0))
                .overflow_y_scroll()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(ThemeColors::BORDER)
                .bg(ThemeColors::BG_SURFACE)
                .child(
                    div()
                        .font_family("monospace")
                        .text_xs()
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(content.clone()),
                )
                .into_any_element()
        } else {
            v_flex()
                .w_full()
                .h(px(320.0))
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(t("export_modal.no_preview_yet", lang)),
                )
                .into_any_element()
        };

        v_flex()
            .w_full()
            .gap_4()
            .p_6()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("export_modal.preview_title", lang)),
                    )
                    .child(refresh_btn),
            )
            .child(preview_body)
            .child(
                h_flex()
                    .justify_between()
                    .pt_3()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .child(back_btn)
                    .child(start_export_btn),
            )
    }

    fn render_step3_progress(&self) -> impl IntoElement {
        let lang = self.lang;
        let on_close = self.on_close.clone();
        let on_reveal = self.on_reveal_file.clone();
        let on_step = self.on_select_step.clone();

        if self.is_executing {
            // Executing view
            return v_flex()
                .w_full()
                .h(px(340.0))
                .items_center()
                .justify_center()
                .gap_4()
                .p_6()
                .child(Spinner::new())
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(t("export_modal.exporting_title", lang)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(format!(
                            "{} {} {}",
                            t("export_modal.processed_rows", lang),
                            self.progress_rows,
                            t("export_modal.rows_unit", lang),
                        )),
                );
        }

        if let Some(ref err) = self.error_msg {
            // Error view
            let to_cfg = on_step.clone();
            let retry_btn = Button::new("export_retry_btn")
                .primary()
                .label(t("export_modal.back_btn", lang))
                .on_click(move |_, window, cx| {
                    if let Some(ref h) = to_cfg {
                        h(ExportWizardStep::Step1Config, window, cx);
                    }
                });

            return v_flex()
                .w_full()
                .h(px(340.0))
                .items_center()
                .justify_center()
                .gap_4()
                .p_6()
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size(px(36.0))
                        .text_color(ThemeColors::ERROR),
                )
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::ERROR)
                        .child(t("export_modal.export_failed", lang)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(err.clone()),
                )
                .child(retry_btn);
        }

        // Success View
        let info = self.success_info.clone();
        let rows = info.as_ref().map(|i| i.rows_count).unwrap_or(0);
        let bytes = info.as_ref().map(|i| i.bytes_written).unwrap_or(0);
        let elapsed = info.as_ref().map(|i| i.elapsed_millis).unwrap_or(0);
        let file_path = info.as_ref().and_then(|i| i.file_path.clone());
        let to_clip = info.as_ref().map(|i| i.copied_to_clipboard).unwrap_or(false);

        let mut done_btn = Button::new("export_done_btn")
            .primary()
            .icon(IconName::Check)
            .label(t("export_modal.done_btn", lang));
        if let Some(ref h) = on_close {
            let h = h.clone();
            done_btn = done_btn.on_click(move |_, window, cx| {
                h(window, cx);
            });
        }

        let mut reveal_btn = Button::new("reveal_file_btn")
            .outline()
            .icon(IconName::FolderOpen)
            .label(t("export_modal.reveal_file", lang));
        if let Some(path) = file_path.clone() {
            if let Some(ref h) = on_reveal {
                let h = h.clone();
                reveal_btn = reveal_btn.on_click(move |_, window, cx| {
                    h(path.clone(), window, cx);
                });
            }
        }

        v_flex()
            .w_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .child(
                Icon::new(IconName::CircleCheck)
                    .size(px(48.0))
                    .text_color(ThemeColors::SUCCESS),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(t("export_modal.success_title", lang)),
            )
            .child(
                v_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!(
                                "Exported {rows} rows ({:.2} KB) in {:.2}s",
                                bytes as f64 / 1024.0,
                                elapsed as f64 / 1000.0
                            )),
                    )
                    .when(to_clip, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::PRIMARY)
                                .child(t("export_modal.copied_notice", lang)),
                        )
                    })
                    .when_some(file_path, |this, path| {
                        this.child(
                            div()
                                .text_xs()
                                .font_family("monospace")
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(path),
                        )
                    }),
            )
            .child(
                h_flex()
                    .gap_3()
                    .pt_3()
                    .child(reveal_btn)
                    .child(done_btn),
            )
    }
}

impl RenderOnce for ExportModal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_close = self.on_close.clone();
        let lang = self.lang;

        let close_btn = Button::new("close_export_modal")
            .ghost()
            .small()
            .icon(IconName::X)
            .on_click(move |_, window, cx| {
                if let Some(ref h) = on_close {
                    h(window, cx);
                }
            });

        let body = match self.step {
            ExportWizardStep::Step1Config => self.render_step1_config().into_any_element(),
            ExportWizardStep::Step2Preview => self.render_step2_preview().into_any_element(),
            ExportWizardStep::Step3Progress => self.render_step3_progress().into_any_element(),
        };

        // Modal backdrop overlay
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
                    .max_h(px(640.0))
                    .bg(ThemeColors::BG_APP)
                    .rounded_lg()
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .shadow_lg()
                    .overflow_hidden()
                    // Header
                    .child(
                        h_flex()
                            .w_full()
                            .h(px(48.0))
                            .px_6()
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
                                        Icon::new(IconName::Download)
                                            .size(px(16.0))
                                            .text_color(ThemeColors::PRIMARY),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(t("export_modal.title", lang)),
                                    ),
                            )
                            .child(self.render_step_indicator())
                            .child(close_btn),
                    )
                    // Body
                    .child(body),
            )
    }
}
