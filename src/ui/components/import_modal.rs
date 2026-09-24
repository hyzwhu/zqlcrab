//! Multi-step Data Import Wizard Modal supporting CSV, TSV, and SQL files.
//! Features automatic delimiter & encoding inference, header recognition,
//! column-to-field mapping, live data preview, batch chunk insertion, and error isolation.

use crate::db::import::{
    ColumnMapping, CsvDelimiter, CsvPreviewData, ErrorPolicy, FileEncoding, ImportFormat,
    ImportProgress, ImportResult, SqlPreviewData,
};
use crate::db::types::ColumnInfo;
use crate::settings::AppLanguage;
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
    Anchor, App, ClipboardItem, ElementId, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window, div,
    prelude::FluentBuilder as _, px, rgba,
};
use std::rc::Rc;

/// Wizard step sequence in the data import workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportWizardStep {
    #[default]
    Step1Source,
    Step2Mapping,
    Step3Execution,
    Step4Done,
}

impl ImportWizardStep {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Step1Source => "1. Data Source",
            Self::Step2Mapping => "2. Field Mapping",
            Self::Step3Execution => "3. Execution",
            Self::Step4Done => "4. Complete",
        }
    }
}

#[derive(IntoElement)]
pub struct ImportModal {
    step: ImportWizardStep,
    file_path_input: Entity<InputState>,
    file_path: Option<std::path::PathBuf>,
    format: ImportFormat,
    encoding: FileEncoding,
    delimiter: CsvDelimiter,
    has_headers: bool,
    target_table: Option<String>,
    available_tables: Vec<String>,
    table_columns: Vec<ColumnInfo>,
    csv_preview: Option<CsvPreviewData>,
    sql_preview: Option<SqlPreviewData>,
    mappings: Vec<ColumnMapping>,
    batch_size: usize,
    error_policy: ErrorPolicy,
    is_executing: bool,
    progress: Option<ImportProgress>,
    result: Option<ImportResult>,
    error_msg: Option<String>,
    language: AppLanguage,

    // Callbacks
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_browse_file: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_inspect_file: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_change_step: Option<Rc<dyn Fn(ImportWizardStep, &mut Window, &mut App) + 'static>>,
    on_select_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_delimiter: Option<Rc<dyn Fn(CsvDelimiter, &mut Window, &mut App) + 'static>>,
    on_select_encoding: Option<Rc<dyn Fn(FileEncoding, &mut Window, &mut App) + 'static>>,
    on_toggle_headers: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_update_mapping: Option<Rc<dyn Fn(usize, Option<String>, &mut Window, &mut App) + 'static>>,
    on_select_batch_size: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_select_error_policy: Option<Rc<dyn Fn(ErrorPolicy, &mut Window, &mut App) + 'static>>,
    on_start_import: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_view_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
}

impl ImportModal {
    pub fn new(
        file_path_input: &Entity<InputState>,
        step: ImportWizardStep,
        format: ImportFormat,
        encoding: FileEncoding,
        delimiter: CsvDelimiter,
        has_headers: bool,
        target_table: Option<String>,
        available_tables: Vec<String>,
        table_columns: Vec<ColumnInfo>,
        mappings: Vec<ColumnMapping>,
        batch_size: usize,
        error_policy: ErrorPolicy,
        is_executing: bool,
    ) -> Self {
        Self {
            step,
            file_path_input: file_path_input.clone(),
            file_path: None,
            format,
            encoding,
            delimiter,
            has_headers,
            target_table,
            available_tables,
            table_columns,
            csv_preview: None,
            sql_preview: None,
            mappings,
            batch_size,
            error_policy,
            is_executing,
            progress: None,
            result: None,
            error_msg: None,
            language: AppLanguage::En,
            on_close: None,
            on_browse_file: None,
            on_inspect_file: None,
            on_change_step: None,
            on_select_table: None,
            on_select_delimiter: None,
            on_select_encoding: None,
            on_toggle_headers: None,
            on_update_mapping: None,
            on_select_batch_size: None,
            on_select_error_policy: None,
            on_start_import: None,
            on_view_table: None,
        }
    }

    pub fn file_path(mut self, path: Option<std::path::PathBuf>) -> Self {
        self.file_path = path;
        self
    }

    pub fn csv_preview(mut self, preview: Option<CsvPreviewData>) -> Self {
        self.csv_preview = preview;
        self
    }

    pub fn sql_preview(mut self, preview: Option<SqlPreviewData>) -> Self {
        self.sql_preview = preview;
        self
    }

    pub fn progress(mut self, progress: Option<ImportProgress>) -> Self {
        self.progress = progress;
        self
    }

    pub fn result(mut self, result: Option<ImportResult>) -> Self {
        self.result = result;
        self
    }

    pub fn error(mut self, err: Option<String>) -> Self {
        self.error_msg = err;
        self
    }

    pub fn language(mut self, lang: AppLanguage) -> Self {
        self.language = lang;
        self
    }

    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }

    pub fn on_browse_file<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_browse_file = Some(Rc::new(handler));
        self
    }

    pub fn on_inspect_file<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_inspect_file = Some(Rc::new(handler));
        self
    }

    pub fn on_change_step<F>(mut self, handler: F) -> Self
    where
        F: Fn(ImportWizardStep, &mut Window, &mut App) + 'static,
    {
        self.on_change_step = Some(Rc::new(handler));
        self
    }

    pub fn on_select_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_table = Some(Rc::new(handler));
        self
    }

    pub fn on_select_delimiter<F>(mut self, handler: F) -> Self
    where
        F: Fn(CsvDelimiter, &mut Window, &mut App) + 'static,
    {
        self.on_select_delimiter = Some(Rc::new(handler));
        self
    }

    pub fn on_select_encoding<F>(mut self, handler: F) -> Self
    where
        F: Fn(FileEncoding, &mut Window, &mut App) + 'static,
    {
        self.on_select_encoding = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_headers<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_headers = Some(Rc::new(handler));
        self
    }

    pub fn on_update_mapping<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, Option<String>, &mut Window, &mut App) + 'static,
    {
        self.on_update_mapping = Some(Rc::new(handler));
        self
    }

    pub fn on_select_batch_size<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_select_batch_size = Some(Rc::new(handler));
        self
    }

    pub fn on_select_error_policy<F>(mut self, handler: F) -> Self
    where
        F: Fn(ErrorPolicy, &mut Window, &mut App) + 'static,
    {
        self.on_select_error_policy = Some(Rc::new(handler));
        self
    }

    pub fn on_start_import<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_start_import = Some(Rc::new(handler));
        self
    }

    pub fn on_view_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_view_table = Some(Rc::new(handler));
        self
    }

    fn render_step_indicator(&self) -> impl IntoElement {
        let steps = [
            ImportWizardStep::Step1Source,
            ImportWizardStep::Step2Mapping,
            ImportWizardStep::Step3Execution,
            ImportWizardStep::Step4Done,
        ];

        let cur_step = self.step;
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .px_6()
            .py_2()
            .bg(ThemeColors::BG_SURFACE)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .children(steps.iter().enumerate().map(|(idx, &s)| {
                let is_current = s == cur_step;
                let is_past = (s as usize) < (cur_step as usize);

                let badge_bg = if is_current {
                    ThemeColors::PRIMARY_BORDER
                } else if is_past {
                    ThemeColors::SUCCESS
                } else {
                    ThemeColors::BG_SURFACE_HOVER
                };

                let text_color = if is_current {
                    ThemeColors::TEXT_PRIMARY
                } else if is_past {
                    ThemeColors::TEXT_MUTED
                } else {
                    ThemeColors::TEXT_FAINT
                };

                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(20.0))
                            .h(px(20.0))
                            .rounded_full()
                            .bg(badge_bg)
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(if is_past {
                                "✓".to_string()
                            } else {
                                (idx + 1).to_string()
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(if is_current {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(text_color)
                            .child(s.title()),
                    )
            }))
    }

    fn render_step1_source(&self) -> impl IntoElement {
        let on_browse = self.on_browse_file.clone();
        let on_inspect = self.on_inspect_file.clone();
        let on_sel_table = self.on_select_table.clone();
        let on_sel_delim = self.on_select_delimiter.clone();
        let on_sel_enc = self.on_select_encoding.clone();
        let on_tog_hdr = self.on_toggle_headers.clone();

        let cur_tbl = self.target_table.clone().unwrap_or_default();
        let tables_clone = self.available_tables.clone();

        let table_menu_btn = Button::new("target_table_dropdown")
            .outline()
            .small()
            .icon(IconName::Table)
            .label(if cur_tbl.is_empty() {
                "Select Target Table".to_string()
            } else {
                cur_tbl.clone()
            })
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for tbl in &tables_clone {
                    let tbl_name = tbl.clone();
                    let sel = on_sel_table.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tbl.clone())
                            .icon(IconName::Table)
                            .on_click(move |_, window, cx| {
                                if let Some(ref handler) = sel {
                                    handler(tbl_name.clone(), window, cx);
                                }
                            }),
                    );
                }
                menu
            });

        let cur_delim = self.delimiter;
        let delim_menu_btn = Button::new("delimiter_dropdown")
            .outline()
            .small()
            .label(cur_delim.display_name())
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for &delim in CsvDelimiter::all() {
                    let sel = on_sel_delim.clone();
                    menu = menu.item(PopupMenuItem::new(delim.display_name()).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = sel {
                                handler(delim, window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        let cur_enc = self.encoding;
        let enc_menu_btn = Button::new("encoding_dropdown")
            .outline()
            .small()
            .label(cur_enc.display_name())
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for &enc in FileEncoding::all() {
                    let sel = on_sel_enc.clone();
                    menu = menu.item(PopupMenuItem::new(enc.display_name()).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = sel {
                                handler(enc, window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        let mut browse_btn = Button::new("browse_file_btn")
            .outline()
            .small()
            .icon(IconName::FolderOpen)
            .label("Browse...");
        if let Some(on_browse) = on_browse {
            browse_btn = browse_btn.on_click(move |_, window, cx| {
                on_browse(window, cx);
            });
        }

        let mut parse_btn = Button::new("inspect_file_btn")
            .primary()
            .small()
            .icon(IconName::RotateCw)
            .label("Analyze File");
        if let Some(on_inspect) = on_inspect {
            parse_btn = parse_btn.on_click(move |_, window, cx| {
                on_inspect(window, cx);
            });
        }

        let has_hdr = self.has_headers;
        let header_toggle_btn = Button::new("toggle_header_btn")
            .outline()
            .small()
            .icon(if has_hdr {
                IconName::Check
            } else {
                IconName::X
            })
            .label(if has_hdr {
                "First line contains column names"
            } else {
                "No header (auto-generate column_1...)"
            })
            .when_some(on_tog_hdr, move |btn, handler| {
                btn.on_click(move |_, window, cx| {
                    handler(!has_hdr, window, cx);
                })
            });

        v_flex()
            .w_full()
            .gap_4()
            .p_6()
            // 1. File picker section
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("DATA FILE PATH (.csv, .tsv, .sql)"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.file_path_input).small().w_full()),
                            )
                            .child(browse_btn)
                            .child(parse_btn),
                    )
                    .when_some(self.file_path.as_ref(), |this, p| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_FAINT)
                                .child(format!("Selected: {}", p.display())),
                        )
                    }),
            )
            // 2. Format & target table section
            .child(
                h_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("FILE FORMAT"),
                            )
                            .child(
                                h_flex()
                                    .h(px(32.0))
                                    .px_3()
                                    .items_center()
                                    .gap_2()
                                    .rounded_md()
                                    .bg(ThemeColors::BG_SURFACE)
                                    .border_1()
                                    .border_color(ThemeColors::BORDER)
                                    .child(
                                        Icon::new(if self.format == ImportFormat::Sql {
                                            IconName::FileCode
                                        } else {
                                            IconName::FileSpreadsheet
                                        })
                                        .size(px(14.0))
                                        .text_color(ThemeColors::PRIMARY_LIGHT),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(self.format.display_name()),
                                    ),
                            ),
                    )
                    .when(self.format != ImportFormat::Sql, |this| {
                        this.child(
                            v_flex()
                                .flex_1()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                        .child("TARGET DATABASE TABLE"),
                                )
                                .child(table_menu_btn),
                        )
                    }),
            )
            // 3. Delimiter & Encoding (only for CSV/TSV)
            .when(self.format != ImportFormat::Sql, |this| {
                this.child(
                    h_flex()
                        .gap_4()
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                        .child("FIELD DELIMITER"),
                                )
                                .child(delim_menu_btn),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                        .child("CHARACTER ENCODING"),
                                )
                                .child(enc_menu_btn),
                        ),
                )
                .child(
                    v_flex()
                        .gap_1p5()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child("HEADER ROW SETTING"),
                        )
                        .child(header_toggle_btn),
                )
            })
    }

    fn render_step2_mapping(&self) -> impl IntoElement {
        let on_update = self.on_update_mapping.clone();
        let cols = self.table_columns.clone();

        v_flex()
            .w_full()
            .gap_4()
            .p_6()
            // Data Preview Table (top 5 rows)
            .when_some(self.csv_preview.as_ref(), |this, preview| {
                this.child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(format!(
                                    "DATA PREVIEW (First {} rows, ~{} estimated total rows)",
                                    preview.sample_rows.len(),
                                    preview.estimated_row_count
                                )),
                        )
                        .child(
                            div()
                                .id("import_preview_scroll")
                                .w_full()
                                .max_h(px(140.0))
                                .overflow_x_scroll()
                                .overflow_y_scroll()
                                .rounded_md()
                                .border_1()
                                .border_color(ThemeColors::BORDER)
                                .bg(ThemeColors::BG_SURFACE)
                                .child(
                                    v_flex()
                                        .min_w_full()
                                        // Header row
                                        .child(
                                            h_flex()
                                                .h(px(26.0))
                                                .bg(ThemeColors::BG_SURFACE_HOVER)
                                                .border_b_1()
                                                .border_color(ThemeColors::BORDER)
                                                .children(preview.headers.iter().map(|h| {
                                                    div()
                                                        .w(px(120.0))
                                                        .px_2()
                                                        .py_1()
                                                        .text_xs()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(h.clone())
                                                })),
                                        )
                                        // Sample rows
                                        .children(preview.sample_rows.iter().take(5).map(|row| {
                                            h_flex()
                                                .h(px(24.0))
                                                .border_b_1()
                                                .border_color(ThemeColors::BORDER)
                                                .children(row.iter().map(|cell| {
                                                    div()
                                                        .w(px(120.0))
                                                        .px_2()
                                                        .py_0p5()
                                                        .text_xs()
                                                        .text_color(ThemeColors::TEXT_MUTED)
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(cell.clone())
                                                }))
                                        })),
                                ),
                        ),
                )
            })
            // SQL script preview
            .when_some(self.sql_preview.as_ref(), |this, preview| {
                this.child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(format!(
                                    "SQL SCRIPT STATEMENTS ({} statements parsed)",
                                    preview.statement_count
                                )),
                        )
                        .child(
                            v_flex()
                                .id("import_sql_stmt_scroll")
                                .w_full()
                                .max_h(px(180.0))
                                .overflow_y_scroll()
                                .gap_1()
                                .p_2()
                                .rounded_md()
                                .border_1()
                                .border_color(ThemeColors::BORDER)
                                .bg(ThemeColors::BG_SURFACE)
                                .children(preview.sample_statements.iter().map(|stmt| {
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(ThemeColors::BG_SURFACE_HOVER)
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                        .child(stmt.clone())
                                })),
                        ),
                )
            })
            // Column Mapping Grid (only CSV/TSV)
            .when(self.format != ImportFormat::Sql, |this| {
                this.child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child("FIELD TO COLUMN MAPPINGS"),
                        )
                        .child(
                            v_flex()
                                .id("import_col_mappings_scroll")
                                .w_full()
                                .max_h(px(200.0))
                                .overflow_y_scroll()
                                .gap_1p5()
                                .p_2()
                                .rounded_md()
                                .border_1()
                                .border_color(ThemeColors::BORDER)
                                .bg(ThemeColors::BG_SURFACE)
                                .children(self.mappings.iter().map(|m| {
                                    let src_idx = m.source_index;
                                    let src_name = m.source_header.clone();
                                    let cur_target = m.target_column.clone();
                                    let on_upd = on_update.clone();
                                    let table_cols_clone = cols.clone();

                                    let target_label = cur_target
                                        .clone()
                                        .unwrap_or_else(|| "(Skip Column)".to_string());
                                    let is_skipped = cur_target.is_none();

                                    let target_dropdown = Button::new(ElementId::Name(
                                        format!("map_dropdown_{}", src_idx).into(),
                                    ))
                                    .outline()
                                    .xsmall()
                                    .label(target_label)
                                    .dropdown_menu_with_anchor(
                                        Anchor::BottomLeft,
                                        move |mut menu, _, _| {
                                            let upd_skip = on_upd.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new("(Skip Column)").on_click(
                                                    move |_, window, cx| {
                                                        if let Some(ref handler) = upd_skip {
                                                            handler(src_idx, None, window, cx);
                                                        }
                                                    },
                                                ),
                                            );

                                            for col in &table_cols_clone {
                                                let upd_col = on_upd.clone();
                                                let col_name = col.name.clone();
                                                let label =
                                                    format!("{} ({})", col.name, col.data_type);
                                                menu =
                                                    menu.item(PopupMenuItem::new(label).on_click(
                                                        move |_, window, cx| {
                                                            if let Some(ref handler) = upd_col {
                                                                handler(
                                                                    src_idx,
                                                                    Some(col_name.clone()),
                                                                    window,
                                                                    cx,
                                                                );
                                                            }
                                                        },
                                                    ));
                                            }
                                            menu
                                        },
                                    );

                                    h_flex()
                                        .w_full()
                                        .px_3()
                                        .py_1p5()
                                        .items_center()
                                        .justify_between()
                                        .rounded_sm()
                                        .bg(ThemeColors::BG_SURFACE_HOVER)
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    Icon::new(IconName::FileText)
                                                        .size(px(13.0))
                                                        .text_color(ThemeColors::PRIMARY_LIGHT),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                                        .child(src_name),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    Icon::new(IconName::ArrowRight)
                                                        .size(px(12.0))
                                                        .text_color(if is_skipped {
                                                            ThemeColors::TEXT_FAINT
                                                        } else {
                                                            ThemeColors::SUCCESS
                                                        }),
                                                )
                                                .child(target_dropdown),
                                        )
                                })),
                        ),
                )
            })
    }

    fn render_step3_execution(&self) -> impl IntoElement {
        let on_start = self.on_start_import.clone();
        let on_sel_batch = self.on_select_batch_size.clone();
        let on_sel_policy = self.on_select_error_policy.clone();

        let cur_batch = self.batch_size;
        let batch_btn = Button::new("batch_size_dropdown")
            .outline()
            .small()
            .label(format!("{cur_batch} rows / batch"))
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for &size in &[100, 500, 1000, 2000] {
                    let sel = on_sel_batch.clone();
                    menu = menu.item(PopupMenuItem::new(format!("{size} rows / batch")).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = sel {
                                handler(size, window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        let cur_policy = self.error_policy;
        let policy_btn = Button::new("error_policy_dropdown")
            .outline()
            .small()
            .label(cur_policy.display_name())
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for &pol in &[ErrorPolicy::Skip, ErrorPolicy::Abort] {
                    let sel = on_sel_policy.clone();
                    menu = menu.item(PopupMenuItem::new(pol.display_name()).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = sel {
                                handler(pol, window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        let mut start_btn = Button::new("start_import_btn")
            .primary()
            .icon(if self.is_executing {
                IconName::RotateCw
            } else {
                IconName::Play
            })
            .label(if self.is_executing {
                "Importing Data..."
            } else {
                "Start Batch Import"
            });

        if !self.is_executing {
            if let Some(on_start) = on_start {
                start_btn = start_btn.on_click(move |_, window, cx| {
                    on_start(window, cx);
                });
            }
        }

        v_flex()
            .w_full()
            .gap_4()
            .p_6()
            // Execution options
            .child(
                h_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("BATCH CHUNK SIZE"),
                            )
                            .child(batch_btn),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("ERROR ISOLATION POLICY"),
                            )
                            .child(policy_btn),
                    ),
            )
            // Progress Section
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .rounded_md()
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(if self.is_executing {
                                        Spinner::new().into_any_element()
                                    } else {
                                        Icon::new(IconName::Database)
                                            .size(px(14.0))
                                            .text_color(ThemeColors::PRIMARY_LIGHT)
                                            .into_any_element()
                                    })
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(if self.is_executing {
                                                "IMPORTING IN PROGRESS..."
                                            } else {
                                                "READY TO EXECUTE"
                                            }),
                                    ),
                            )
                            .child(start_btn),
                    )
                    // Progress bar & metrics
                    .when_some(self.progress.as_ref(), |this, prog| {
                        let pct = if prog.total_estimated_rows > 0 {
                            ((prog.processed_rows as f32 / prog.total_estimated_rows as f32)
                                * 100.0)
                                .min(100.0)
                        } else {
                            100.0
                        };

                        this.child(
                            v_flex()
                                .gap_1p5()
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(8.0))
                                        .rounded_full()
                                        .bg(ThemeColors::BG_SURFACE_HOVER)
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .h_full()
                                                .rounded_full()
                                                .bg(ThemeColors::PRIMARY_BORDER)
                                                .w(px(pct * 6.5)), // 650px full width approximation
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(ThemeColors::TEXT_MUTED)
                                                .child(format!(
                                                    "Processed {} of ~{} rows ({:.1}%)",
                                                    prog.processed_rows,
                                                    prog.total_estimated_rows,
                                                    pct
                                                )),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(ThemeColors::SUCCESS)
                                                .child(format!(
                                                    "Throughput: {} rows/s (Elapsed: {:.1}s)",
                                                    prog.throughput_rows_per_sec, prog.elapsed_secs
                                                )),
                                        ),
                                ),
                        )
                    }),
            )
    }

    fn render_step4_done(&self) -> impl IntoElement {
        let on_view = self.on_view_table.clone();
        let cur_tbl = self.target_table.clone().unwrap_or_default();

        let mut view_table_btn = Button::new("done_view_table_btn")
            .primary()
            .small()
            .icon(IconName::Table)
            .label(format!("View Table '{cur_tbl}'"));

        if let Some(on_view) = on_view {
            view_table_btn = view_table_btn.on_click(move |_, window, cx| {
                on_view(cur_tbl.clone(), window, cx);
            });
        }

        let res = self.result.clone();
        let total_succ = res.as_ref().map(|r| r.total_succeeded).unwrap_or(0);
        let total_fail = res.as_ref().map(|r| r.total_failed).unwrap_or(0);
        let elapsed_ms = res.as_ref().map(|r| r.elapsed_millis).unwrap_or(0);
        let error_rows = res.map(|r| r.error_rows).unwrap_or_default();

        let err_log_text = error_rows
            .iter()
            .map(|e| {
                format!(
                    "Line {}: {}\nReason: {}",
                    e.line_number, e.raw_data, e.reason
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let copy_log_btn = Button::new("copy_error_log_btn")
            .outline()
            .xsmall()
            .icon(IconName::Copy)
            .label("Copy Error Log")
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(err_log_text.clone()));
            });

        v_flex()
            .w_full()
            .gap_4()
            .p_6()
            // Success summary banner
            .child(
                h_flex()
                    .w_full()
                    .p_4()
                    .items_center()
                    .justify_between()
                    .rounded_md()
                    .bg(if total_fail == 0 {
                        ThemeColors::BG_SURFACE
                    } else {
                        ThemeColors::BG_SURFACE
                    })
                    .border_1()
                    .border_color(if total_fail == 0 {
                        ThemeColors::SUCCESS
                    } else {
                        ThemeColors::WARNING
                    })
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Icon::new(if total_fail == 0 {
                                    IconName::CircleCheck
                                } else {
                                    IconName::TriangleAlert
                                })
                                .size(px(24.0))
                                .text_color(if total_fail == 0 {
                                    ThemeColors::SUCCESS
                                } else {
                                    ThemeColors::WARNING
                                }),
                            )
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(if total_fail == 0 {
                                                "Import Completed Successfully"
                                            } else {
                                                "Import Completed with Isolated Errors"
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(format!(
                                                "Successfully inserted {} rows in {:.2}s. (Failed: {} rows)",
                                                total_succ,
                                                elapsed_ms as f64 / 1000.0,
                                                total_fail
                                            )),
                                    ),
                            ),
                    )
                    .child(view_table_btn),
            )
            // Error log list (if errors occurred)
            .when(!error_rows.is_empty(), |this| {
                this.child(
                    v_flex()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(ThemeColors::ERROR)
                                        .child(format!("ISOLATED ERROR ROWS ({})", error_rows.len())),
                                )
                                .child(copy_log_btn),
                        )
                        .child(
                            v_flex()
                                .id("import_error_rows_scroll")
                                .w_full()
                                .max_h(px(160.0))
                                .overflow_y_scroll()
                                .gap_1()
                                .p_2()
                                .rounded_md()
                                .border_1()
                                .border_color(ThemeColors::BORDER)
                                .bg(ThemeColors::BG_SURFACE)
                                .children(error_rows.iter().take(20).map(|err| {
                                    v_flex()
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(ThemeColors::BG_SURFACE_HOVER)
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(ThemeColors::ERROR)
                                                        .child(format!("Line {}", err.line_number)),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(err.raw_data.clone()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(ThemeColors::TEXT_MUTED)
                                                .child(err.reason.clone()),
                                        )
                                })),
                        ),
                )
            })
    }
}

impl RenderOnce for ImportModal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let on_close_action = self.on_close.clone();
        let on_step_action = self.on_change_step.clone();

        let mut close_btn = Button::new("close_import_modal")
            .ghost()
            .xsmall()
            .icon(IconName::X);
        if let Some(ref on_close) = on_close_action {
            let on_close = on_close.clone();
            close_btn = close_btn.on_click(move |_, window, cx| {
                on_close(window, cx);
            });
        }

        let mut cancel_btn = Button::new("cancel_import_btn")
            .outline()
            .small()
            .label("Cancel");
        if let Some(ref on_close) = on_close_action {
            let on_close = on_close.clone();
            cancel_btn = cancel_btn.on_click(move |_, window, cx| {
                on_close(window, cx);
            });
        }

        let cur_step = self.step;

        // Navigation buttons (Back / Next)
        let prev_step = match cur_step {
            ImportWizardStep::Step2Mapping => Some(ImportWizardStep::Step1Source),
            ImportWizardStep::Step3Execution => {
                if self.format == ImportFormat::Sql {
                    Some(ImportWizardStep::Step1Source)
                } else {
                    Some(ImportWizardStep::Step2Mapping)
                }
            }
            _ => None,
        };

        let next_step = match cur_step {
            ImportWizardStep::Step1Source => {
                if self.format == ImportFormat::Sql {
                    Some(ImportWizardStep::Step3Execution)
                } else {
                    Some(ImportWizardStep::Step2Mapping)
                }
            }
            ImportWizardStep::Step2Mapping => Some(ImportWizardStep::Step3Execution),
            ImportWizardStep::Step3Execution => {
                if self.result.is_some() {
                    Some(ImportWizardStep::Step4Done)
                } else {
                    None
                }
            }
            ImportWizardStep::Step4Done => None,
        };

        let mut back_btn = Button::new("back_step_btn")
            .outline()
            .small()
            .icon(IconName::ChevronLeft)
            .label("Back");
        if let Some(prev) = prev_step {
            if let Some(ref on_step) = on_step_action {
                let on_step = on_step.clone();
                back_btn = back_btn.on_click(move |_, window, cx| {
                    on_step(prev, window, cx);
                });
            }
        }

        let mut next_btn = Button::new("next_step_btn")
            .primary()
            .small()
            .icon(IconName::ChevronRight)
            .label("Next");
        if let Some(next) = next_step {
            if let Some(ref on_step) = on_step_action {
                let on_step = on_step.clone();
                next_btn = next_btn.on_click(move |_, window, cx| {
                    on_step(next, window, cx);
                });
            }
        }

        let step_content = match self.step {
            ImportWizardStep::Step1Source => self.render_step1_source().into_any_element(),
            ImportWizardStep::Step2Mapping => self.render_step2_mapping().into_any_element(),
            ImportWizardStep::Step3Execution => self.render_step3_execution().into_any_element(),
            ImportWizardStep::Step4Done => self.render_step4_done().into_any_element(),
        };

        // Modal backdrop overlay with .occlude() to prevent mouse pass-through
        div()
            .id("import_modal_backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000B0))
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                v_flex()
                    .id("import_modal_card")
                    .w(px(720.0))
                    .max_h(px(640.0))
                    .rounded_xl()
                    .bg(ThemeColors::BG_APP)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .shadow_lg()
                    .overflow_hidden()
                    // Header
                    .child(
                        h_flex()
                            .h(px(46.0))
                            .w_full()
                            .px_6()
                            .items_center()
                            .justify_between()
                            .bg(ThemeColors::BG_SURFACE)
                            .border_b_1()
                            .border_color(ThemeColors::BORDER)
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::new(IconName::Upload)
                                            .size(px(16.0))
                                            .text_color(ThemeColors::PRIMARY_LIGHT),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child("Data Import Wizard"),
                                    ),
                            )
                            .child(close_btn),
                    )
                    // Step tabs indicator
                    .child(self.render_step_indicator())
                    // Error message banner
                    .when_some(self.error_msg.as_ref(), |this, err| {
                        this.child(
                            h_flex()
                                .w_full()
                                .px_6()
                                .py_2()
                                .items_center()
                                .gap_2()
                                .bg(ThemeColors::BG_SURFACE)
                                .border_b_1()
                                .border_color(ThemeColors::ERROR)
                                .child(
                                    Icon::new(IconName::TriangleAlert)
                                        .size(px(14.0))
                                        .text_color(ThemeColors::ERROR),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::ERROR)
                                        .child(err.clone()),
                                ),
                        )
                    })
                    // Body
                    .child(
                        div()
                            .id("import_modal_body_scroll")
                            .flex_1()
                            .w_full()
                            .overflow_y_scroll()
                            .child(step_content),
                    )
                    // Footer
                    .child(
                        h_flex()
                            .h(px(48.0))
                            .w_full()
                            .px_6()
                            .items_center()
                            .justify_between()
                            .bg(ThemeColors::BG_SURFACE)
                            .border_t_1()
                            .border_color(ThemeColors::BORDER)
                            .child(cancel_btn)
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .when(prev_step.is_some(), |this| this.child(back_btn))
                                    .when(next_step.is_some(), |this| this.child(next_btn)),
                            ),
                    ),
            )
    }
}
