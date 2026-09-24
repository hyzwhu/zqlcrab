//! Visual Mock Data Generator and Batch Seeder Modal.
//! Allows generating realistic test data (Names, Emails, Dates, UUIDs, Ranges, Enums) and seeding in batches.

use crate::db::mock_data::{MockColumnConfig, MockGeneratorType, MockProgress, MockResult};
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
    spinner::Spinner,
};
use gpui_kit::gpui::{
    Anchor, App, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
    px, rgba,
};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MockWizardStep {
    #[default]
    Step1Config,
    Step2Preview,
    Step3Progress,
}

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct MockDataModal {
    step: MockWizardStep,
    target_table: String,
    available_tables: Vec<String>,
    columns: Vec<MockColumnConfig>,
    row_count: usize,
    batch_size: usize,
    preview_headers: Vec<String>,
    preview_rows: Vec<Vec<String>>,
    is_loading_preview: bool,
    is_executing: bool,
    progress: Option<MockProgress>,
    result: Option<MockResult>,
    error_msg: Option<String>,
    lang: AppLanguage,

    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_select_step: Option<Rc<dyn Fn(MockWizardStep, &mut Window, &mut App) + 'static>>,
    on_select_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_row_count: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_select_batch_size: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_change_column_generator:
        Option<Rc<dyn Fn(usize, MockGeneratorType, &mut Window, &mut App) + 'static>>,
    on_refresh_preview: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_start_seeding: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_view_in_grid: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
}

impl MockDataModal {
    pub fn new(
        step: MockWizardStep,
        target_table: String,
        available_tables: Vec<String>,
        columns: Vec<MockColumnConfig>,
        row_count: usize,
        batch_size: usize,
    ) -> Self {
        Self {
            step,
            target_table,
            available_tables,
            columns,
            row_count,
            batch_size,
            preview_headers: Vec::new(),
            preview_rows: Vec::new(),
            is_loading_preview: false,
            is_executing: false,
            progress: None,
            result: None,
            error_msg: None,
            lang: AppLanguage::En,
            on_close: None,
            on_select_step: None,
            on_select_table: None,
            on_select_row_count: None,
            on_select_batch_size: None,
            on_change_column_generator: None,
            on_refresh_preview: None,
            on_start_seeding: None,
            on_view_in_grid: None,
        }
    }

    pub fn preview(
        mut self,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        is_loading: bool,
    ) -> Self {
        self.preview_headers = headers;
        self.preview_rows = rows;
        self.is_loading_preview = is_loading;
        self
    }

    pub fn progress(mut self, p: Option<MockProgress>) -> Self {
        self.progress = p;
        self
    }

    pub fn result(mut self, r: Option<MockResult>) -> Self {
        self.result = r;
        self
    }

    pub fn error(mut self, err: Option<String>) -> Self {
        self.error_msg = err;
        self
    }

    pub fn executing(mut self, is_executing: bool) -> Self {
        self.is_executing = is_executing;
        self
    }

    pub fn language(mut self, lang: AppLanguage) -> Self {
        self.lang = lang;
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
        F: Fn(MockWizardStep, &mut Window, &mut App) + 'static,
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

    pub fn on_select_row_count<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_select_row_count = Some(Rc::new(handler));
        self
    }

    pub fn on_select_batch_size<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_select_batch_size = Some(Rc::new(handler));
        self
    }

    pub fn on_change_column_generator<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, MockGeneratorType, &mut Window, &mut App) + 'static,
    {
        self.on_change_column_generator = Some(Rc::new(handler));
        self
    }

    pub fn on_refresh_preview<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh_preview = Some(Rc::new(handler));
        self
    }

    pub fn on_start_seeding<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_start_seeding = Some(Rc::new(handler));
        self
    }

    pub fn on_view_in_grid<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_view_in_grid = Some(Rc::new(handler));
        self
    }

    fn render_step_indicator(&self) -> impl IntoElement {
        let steps = [
            MockWizardStep::Step1Config,
            MockWizardStep::Step2Preview,
            MockWizardStep::Step3Progress,
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
                    MockWizardStep::Step1Config => t("mock_modal.step_config", lang),
                    MockWizardStep::Step2Preview => t("mock_modal.step_preview", lang),
                    MockWizardStep::Step3Progress => t("mock_modal.step_execute", lang),
                };

                let step_val = *step_item;
                let step_cb = on_step.clone();

                let mut item = div()
                    .id(ElementId::Name(format!("step_pill_{}", idx).into()))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .text_size(px(12.0));

                if is_active {
                    item = item
                        .bg(ThemeColors::PRIMARY_BG)
                        .border_1()
                        .border_color(ThemeColors::PRIMARY_BORDER)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .font_weight(FontWeight::SEMIBOLD);
                } else if is_past {
                    item = item
                        .bg(ThemeColors::BG_SURFACE_HOVER)
                        .text_color(ThemeColors::SUCCESS)
                        .font_weight(FontWeight::MEDIUM);
                } else {
                    item = item
                        .bg(ThemeColors::BG_SURFACE)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .font_weight(FontWeight::NORMAL);
                }

                if !is_executing && (is_past || is_active) {
                    item = item.cursor_pointer().on_click(move |_, window, cx| {
                        if let Some(ref cb) = step_cb {
                            cb(step_val, window, cx);
                        }
                    });
                }

                let badge = div()
                    .w_4()
                    .h_4()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .when(is_active, |d| {
                        d.bg(ThemeColors::PRIMARY)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                    })
                    .when(is_past, |d| {
                        d.bg(ThemeColors::SUCCESS)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                    })
                    .when(!is_active && !is_past, |d| {
                        d.bg(ThemeColors::BG_SURFACE_HOVER)
                            .text_color(ThemeColors::TEXT_MUTED)
                    })
                    .child(if is_past {
                        "✓"
                    } else {
                        (idx + 1).to_string().leak()
                    });

                h_flex()
                    .items_center()
                    .gap_1()
                    .child(item.child(badge).child(label))
                    .when(idx < steps.len() - 1, |d| {
                        d.child(
                            Icon::new(IconName::ChevronRight)
                                .size(px(12.0))
                                .text_color(ThemeColors::TEXT_FAINT),
                        )
                    })
            }))
    }

    fn render_step1_config(&self) -> impl IntoElement {
        let lang = self.lang;
        let on_sel_table = self.on_select_table.clone();
        let on_sel_count = self.on_select_row_count.clone();
        let on_sel_batch = self.on_select_batch_size.clone();
        let on_step = self.on_select_step.clone();
        let on_start = self.on_start_seeding.clone();

        // 1. Target table selector
        let cur_tbl = self.target_table.clone();
        let tables_clone = self.available_tables.clone();
        let table_menu_btn = Button::new("mock_target_table_dropdown")
            .outline()
            .text_size(px(13.0))
            .icon(IconName::Database)
            .child(cur_tbl.clone())
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for tbl in &tables_clone {
                    let tbl_name = tbl.clone();
                    let cb = on_sel_table.clone();
                    menu = menu.item(PopupMenuItem::new(tbl_name.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = cb {
                                handler(tbl_name.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        // 2. Row count pills
        let row_counts = [100, 500, 1000, 5000];
        let current_count = self.row_count;

        // 3. Batch size pills
        let batch_sizes = [100, 200, 500];
        let current_batch = self.batch_size;

        v_flex()
            .gap_4()
            .child(
                // Configuration Top Bar
                h_flex()
                    .gap_6()
                    .items_center()
                    .p_3()
                    .rounded_lg()
                    .bg(ThemeColors::BG_APP)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("mock_modal.target_table", lang)),
                            )
                            .child(table_menu_btn),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("mock_modal.rows_to_generate", lang)),
                            )
                            .child(h_flex().gap_1().children(row_counts.iter().map(|&cnt| {
                                let is_sel = cnt == current_count;
                                let cb = on_sel_count.clone();
                                let mut btn =
                                    Button::new(ElementId::Name(format!("rc_{}", cnt).into()))
                                        .text_size(px(12.0));
                                if is_sel {
                                    btn = btn.primary();
                                } else {
                                    btn = btn.outline();
                                }
                                btn.child(format!("{} rows", cnt))
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref handler) = cb {
                                            handler(cnt, window, cx);
                                        }
                                    })
                            }))),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("mock_modal.batch_size", lang)),
                            )
                            .child(h_flex().gap_1().children(batch_sizes.iter().map(|&bs| {
                                let is_sel = bs == current_batch;
                                let cb = on_sel_batch.clone();
                                let mut btn =
                                    Button::new(ElementId::Name(format!("bs_{}", bs).into()))
                                        .text_size(px(12.0));
                                if is_sel {
                                    btn = btn.primary();
                                } else {
                                    btn = btn.outline();
                                }
                                btn.child(format!("{}", bs)).on_click(move |_, window, cx| {
                                    if let Some(ref handler) = cb {
                                        handler(bs, window, cx);
                                    }
                                })
                            }))),
                    ),
            )
            .child(
                // Column Rules Header & Scrollable List
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("mock_modal.column_rules", lang)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!("{} columns detected", self.columns.len())),
                            ),
                    )
                    .child(self.render_columns_table()),
            )
            .child(
                // Bottom Actions Bar
                h_flex()
                    .justify_between()
                    .items_center()
                    .pt_3()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child("All generated data will be inserted in an atomic transaction."),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                Button::new("mock_preview_btn")
                                    .outline()
                                    .icon(IconName::Eye)
                                    .child(t("mock_modal.preview_btn", lang))
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref cb) = on_step {
                                            cb(MockWizardStep::Step2Preview, window, cx);
                                        }
                                    }),
                            )
                            .child(
                                Button::new("mock_start_seeding_btn")
                                    .primary()
                                    .icon(IconName::Sparkles)
                                    .child(t("mock_modal.start_seeding", lang))
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref cb) = on_start {
                                            cb(window, cx);
                                        }
                                    }),
                            ),
                    ),
            )
    }

    fn render_columns_table(&self) -> impl IntoElement {
        let lang = self.lang;
        let on_change_gen = self.on_change_column_generator.clone();

        v_flex()
            .id("mock_columns_scroll")
            .w_full()
            .max_h(px(320.0))
            .overflow_y_scroll()
            .rounded_lg()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                // Table Header
                h_flex()
                    .p_2()
                    .bg(ThemeColors::BG_APP)
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(div().w(px(160.0)).child(t("mock_modal.col_name", lang)))
                    .child(div().w(px(120.0)).child(t("mock_modal.col_type", lang)))
                    .child(div().flex_1().child(t("mock_modal.col_generator", lang)))
                    .child(div().w(px(80.0)).text_right().child("Status")),
            )
            .children(self.columns.iter().enumerate().map(|(idx, col)| {
                let is_ignored = col.generator.is_ignored();
                let col_name = col.column_name.clone();
                let col_type = col.data_type.clone();
                let is_pk = col.is_pk;
                let gen_clone = col.generator.clone();
                let cb = on_change_gen.clone();

                let generator_menu_btn =
                    Button::new(ElementId::Name(format!("col_gen_{}", idx).into()))
                        .outline()
                        .text_size(px(12.0))
                        .child(gen_clone.display_name())
                        .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                            let options = [
                                MockGeneratorType::AutoIncrement { start: 1, step: 1 },
                                MockGeneratorType::PersonName,
                                MockGeneratorType::Email,
                                MockGeneratorType::PhoneNumber,
                                MockGeneratorType::UuidV4,
                                MockGeneratorType::RandomInt { min: 1, max: 1000 },
                                MockGeneratorType::RandomFloat {
                                    min: 10.0,
                                    max: 1000.0,
                                    decimals: 2,
                                },
                                MockGeneratorType::DateTime { past_days: 30 },
                                MockGeneratorType::Date { past_days: 365 },
                                MockGeneratorType::Boolean { true_ratio: 0.8 },
                                MockGeneratorType::EnumChoices {
                                    options: vec![
                                        "active".into(),
                                        "pending".into(),
                                        "inactive".into(),
                                    ],
                                },
                                MockGeneratorType::LoremIpsum { words: 6 },
                                MockGeneratorType::Null,
                                MockGeneratorType::Ignored,
                            ];

                            for opt in options {
                                let opt_type = opt.clone();
                                let opt_cb = cb.clone();
                                menu = menu.item(PopupMenuItem::new(opt.display_name()).on_click(
                                    move |_, window, cx| {
                                        if let Some(ref handler) = opt_cb {
                                            handler(idx, opt_type.clone(), window, cx);
                                        }
                                    },
                                ));
                            }
                            menu
                        });

                h_flex()
                    .items_center()
                    .p_2()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER_LIGHT)
                    .when(is_ignored, |d| d.opacity(0.55).bg(ThemeColors::BG_APP))
                    .child(
                        h_flex()
                            .w(px(160.0))
                            .gap_1()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(col_name),
                            )
                            .when(is_pk, |d| {
                                d.child(
                                    div()
                                        .px_1()
                                        .rounded_sm()
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::BOLD)
                                        .bg(ThemeColors::BG_SURFACE_HOVER)
                                        .text_color(ThemeColors::WARNING)
                                        .child("PK"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .w(px(120.0))
                            .text_size(px(11.0))
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(col_type),
                    )
                    .child(div().flex_1().child(generator_menu_btn))
                    .child(div().w(px(80.0)).text_right().child(if is_ignored {
                        div()
                            .text_size(px(11.0))
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child("Ignored")
                    } else {
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ThemeColors::SUCCESS)
                            .child("Active")
                    }))
            }))
    }

    fn render_step2_preview(&self) -> impl IntoElement {
        let lang = self.lang;
        let on_refresh = self.on_refresh_preview.clone();
        let on_step = self.on_select_step.clone();
        let on_start = self.on_start_seeding.clone();
        let is_loading = self.is_loading_preview;

        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("Sample Generated Data Preview (First 8 Rows)"),
                    )
                    .child(
                        Button::new("refresh_mock_preview")
                            .outline()
                            .icon(IconName::RotateCcw)
                            .child(t("mock_modal.refresh_preview", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref cb) = on_refresh {
                                    cb(window, cx);
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .id("mock_preview_scroll")
                    .w_full()
                    .h(px(320.0))
                    .rounded_lg()
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .overflow_y_scroll()
                    .when(is_loading, |d| {
                        d.flex().items_center().justify_center().child(
                            h_flex().gap_2().items_center().child(Spinner::new()).child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child("Generating sample rows..."),
                            ),
                        )
                    })
                    .when(!is_loading && self.preview_rows.is_empty(), |d| {
                        d.flex().items_center().justify_center().child(
                            div()
                                .text_size(px(13.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child("No columns configured or all columns ignored."),
                        )
                    })
                    .when(!is_loading && !self.preview_rows.is_empty(), |d| {
                        d.child(
                            v_flex()
                                .w_full()
                                .child(
                                    // Headers
                                    h_flex()
                                        .p_2()
                                        .bg(ThemeColors::BG_APP)
                                        .border_b_1()
                                        .border_color(ThemeColors::BORDER)
                                        .children(self.preview_headers.iter().map(|h| {
                                            div()
                                                .w(px(150.0))
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(ThemeColors::TEXT_MUTED)
                                                .child(h.clone())
                                        })),
                                )
                                .children(self.preview_rows.iter().enumerate().map(
                                    |(idx, row)| {
                                        h_flex()
                                            .p_2()
                                            .border_b_1()
                                            .border_color(ThemeColors::BORDER_LIGHT)
                                            .when(idx % 2 == 1, |d| d.bg(ThemeColors::BG_APP))
                                            .children(row.iter().map(|val| {
                                                div()
                                                    .w(px(150.0))
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::NORMAL)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .overflow_hidden()
                                                    .child(val.clone())
                                            }))
                                    },
                                )),
                        )
                    }),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .pt_3()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        Button::new("mock_back_btn")
                            .outline()
                            .icon(IconName::ChevronLeft)
                            .child(t("mock_modal.back", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref cb) = on_step {
                                    cb(MockWizardStep::Step1Config, window, cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("mock_preview_start_btn")
                            .primary()
                            .icon(IconName::Sparkles)
                            .child(t("mock_modal.start_seeding", lang))
                            .on_click(move |_, window, cx| {
                                if let Some(ref cb) = on_start {
                                    cb(window, cx);
                                }
                            }),
                    ),
            )
    }

    fn render_step3_progress(&self) -> impl IntoElement {
        let lang = self.lang;
        let on_close = self.on_close.clone();
        let on_grid = self.on_view_in_grid.clone();
        let target_tbl = self.target_table.clone();
        let is_executing = self.is_executing;
        let error = self.error_msg.clone();
        let result = self.result.clone();
        let progress = self.progress;

        v_flex()
            .gap_6()
            .py_4()
            .items_center()
            .justify_center()
            .when(is_executing, |d| {
                let percent = progress.map(|p| p.percent).unwrap_or(0.0);
                let inserted = progress.map(|p| p.inserted).unwrap_or(0);
                let total = progress.map(|p| p.total).unwrap_or(self.row_count);

                d.child(
                    v_flex()
                        .gap_4()
                        .w_full()
                        .max_w(px(520.0))
                        .items_center()
                        .child(Spinner::new())
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child("Seeding Mock Data..."),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(t("mock_modal.seeding_in_progress", lang)),
                        )
                        .child(
                            // Progress bar
                            div()
                                .w_full()
                                .h_3()
                                .rounded_full()
                                .bg(ThemeColors::BG_APP)
                                .border_1()
                                .border_color(ThemeColors::BORDER)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .bg(ThemeColors::PRIMARY)
                                        .w(px(5.2 * percent.clamp(0.0, 100.0))),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(format!("{}/{} rows ({:.1}%)", inserted, total, percent)),
                        ),
                )
            })
            .when(!is_executing && error.is_some(), |d| {
                let err_msg = error.unwrap();
                let on_step = self.on_select_step.clone();
                d.child(
                    v_flex()
                        .gap_4()
                        .w_full()
                        .max_w(px(520.0))
                        .items_center()
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size(px(40.0))
                                .text_color(ThemeColors::ERROR),
                        )
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::ERROR)
                                .child(t("mock_modal.failed", lang)),
                        )
                        .child(
                            div()
                                .p_3()
                                .rounded_lg()
                                .bg(ThemeColors::BG_APP)
                                .border_1()
                                .border_color(ThemeColors::BORDER)
                                .text_size(px(12.0))
                                .text_color(ThemeColors::ERROR)
                                .child(err_msg),
                        )
                        .child(
                            Button::new("mock_err_back_btn")
                                .outline()
                                .child(t("mock_modal.back", lang))
                                .on_click(move |_, window, cx| {
                                    if let Some(ref cb) = on_step {
                                        cb(MockWizardStep::Step1Config, window, cx);
                                    }
                                }),
                        ),
                )
            })
            .when(!is_executing && result.is_some(), |d| {
                let res = result.unwrap();
                d.child(
                    v_flex()
                        .gap_4()
                        .w_full()
                        .max_w(px(520.0))
                        .items_center()
                        .child(
                            div()
                                .w_12()
                                .h_12()
                                .rounded_full()
                                .bg(ThemeColors::BG_SURFACE_HOVER)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Check)
                                        .size(px(24.0))
                                        .text_color(ThemeColors::SUCCESS),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(17.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(t("mock_modal.success_title", lang)),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(t("mock_modal.success_desc", lang)),
                        )
                        .child(
                            // Summary Cards
                            h_flex()
                                .gap_4()
                                .child(
                                    v_flex()
                                        .p_3()
                                        .rounded_lg()
                                        .bg(ThemeColors::BG_APP)
                                        .border_1()
                                        .border_color(ThemeColors::BORDER)
                                        .items_center()
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(ThemeColors::TEXT_MUTED)
                                                .child(t("mock_modal.rows_inserted", lang)),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(18.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(ThemeColors::TEXT_PRIMARY)
                                                .child(res.total_inserted.to_string()),
                                        ),
                                )
                                .child(
                                    v_flex()
                                        .p_3()
                                        .rounded_lg()
                                        .bg(ThemeColors::BG_APP)
                                        .border_1()
                                        .border_color(ThemeColors::BORDER)
                                        .items_center()
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(ThemeColors::TEXT_MUTED)
                                                .child(t("mock_modal.time_elapsed", lang)),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(18.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(ThemeColors::TEXT_PRIMARY)
                                                .child(format!(
                                                    "{:.2}s",
                                                    res.elapsed_ms as f64 / 1000.0
                                                )),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_3()
                                .pt_2()
                                .child(
                                    Button::new("mock_done_close_btn")
                                        .outline()
                                        .child(t("mock_modal.close", lang))
                                        .on_click(move |_, window, cx| {
                                            if let Some(ref cb) = on_close {
                                                cb(window, cx);
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("mock_done_view_grid_btn")
                                        .primary()
                                        .icon(IconName::Table)
                                        .child(t("mock_modal.view_in_grid", lang))
                                        .on_click(move |_, window, cx| {
                                            if let Some(ref cb) = on_grid {
                                                cb(target_tbl.clone(), window, cx);
                                            }
                                        }),
                                ),
                        ),
                )
            })
    }
}

impl RenderOnce for MockDataModal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let lang = self.lang;
        let on_close = self.on_close.clone();
        let is_executing = self.is_executing;

        // Modal backdrop overlay
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000a6))
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(760.0))
                    .max_h(px(680.0))
                    .rounded_xl()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .shadow_2xl()
                    .overflow_hidden()
                    .child(
                        // Header
                        h_flex()
                            .justify_between()
                            .items_center()
                            .p_4()
                            .border_b_1()
                            .border_color(ThemeColors::BORDER)
                            .bg(ThemeColors::BG_APP)
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        div()
                                            .w_8()
                                            .h_8()
                                            .rounded_lg()
                                            .bg(ThemeColors::PRIMARY_BG)
                                            .border_1()
                                            .border_color(ThemeColors::PRIMARY_BORDER)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                Icon::new(IconName::Sparkles)
                                                    .size(px(18.0))
                                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_size(px(15.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child(t("mock_modal.title", lang)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(ThemeColors::TEXT_MUTED)
                                                    .child(t("mock_modal.subtitle", lang)),
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(self.render_step_indicator())
                                    .when(!is_executing, |d| {
                                        d.child(
                                            Button::new("mock_modal_close_btn")
                                                .ghost()
                                                .icon(IconName::X)
                                                .on_click(move |_, window, cx| {
                                                    if let Some(ref cb) = on_close {
                                                        cb(window, cx);
                                                    }
                                                }),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        // Body view depending on active wizard step
                        div().p_5().child(match self.step {
                            MockWizardStep::Step1Config => {
                                self.render_step1_config().into_any_element()
                            }
                            MockWizardStep::Step2Preview => {
                                self.render_step2_preview().into_any_element()
                            }
                            MockWizardStep::Step3Progress => {
                                self.render_step3_progress().into_any_element()
                            }
                        }),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_wizard_step_default() {
        assert_eq!(MockWizardStep::default(), MockWizardStep::Step1Config);
    }

    #[test]
    fn test_mock_modal_builder() {
        let cols = vec![MockColumnConfig {
            column_name: "id".to_string(),
            data_type: "INT".to_string(),
            is_pk: true,
            is_nullable: false,
            generator: MockGeneratorType::AutoIncrement { start: 1, step: 1 },
        }];
        let modal = MockDataModal::new(
            MockWizardStep::Step1Config,
            "users".to_string(),
            vec!["users".to_string(), "orders".to_string()],
            cols,
            500,
            200,
        )
        .language(AppLanguage::ZhCn)
        .preview(vec!["id".into()], vec![vec!["1".into()]], false)
        .progress(Some(MockProgress {
            inserted: 100,
            total: 500,
            percent: 20.0,
        }))
        .result(Some(MockResult {
            total_inserted: 500,
            elapsed_ms: 120,
        }))
        .executing(true);

        assert_eq!(modal.step, MockWizardStep::Step1Config);
        assert_eq!(modal.target_table, "users");
        assert_eq!(modal.row_count, 500);
        assert_eq!(modal.batch_size, 200);
        assert!(modal.is_executing);
    }
}
