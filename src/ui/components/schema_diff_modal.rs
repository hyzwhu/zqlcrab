//! Visual Schema Diff and Migration Script Generator Modal Dialog.
//! Allows inspecting structural discrepancies between two tables (columns, types, nullability, defaults, PKs, indexes)
//! side-by-side and generating dialect-specific non-destructive or transactional migration DDL scripts.

use crate::db::schema_diff::{
    ColumnDiff, ColumnDiffStatus, IndexDiff, IndexDiffStatus, MigrationDirection, MigrationScript,
    SchemaDiffOptions, SchemaDiffReport,
};
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::gpui::{
    Anchor, App, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
    px, rgba,
};
use std::rc::Rc;

/// Active filter tab for the discrepancy matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffFilterTab {
    #[default]
    DifferencesOnly,
    All,
    ColumnsOnly,
    IndexesOnly,
}

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct SchemaDiffModal {
    source_table: String,
    target_table: String,
    available_tables: Vec<String>,
    report: SchemaDiffReport,
    options: SchemaDiffOptions,
    migration_script: MigrationScript,
    filter_tab: DiffFilterTab,
    copied: bool,
    language: AppLanguage,
    on_select_source_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_target_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_swap_tables: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_change_filter: Option<Rc<dyn Fn(DiffFilterTab, &mut Window, &mut App) + 'static>>,
    on_toggle_safe_mode: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_toggle_wrap_tx: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_change_direction: Option<Rc<dyn Fn(MigrationDirection, &mut Window, &mut App) + 'static>>,
    on_copy_sql: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_open_in_console: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl SchemaDiffModal {
    pub fn new(
        source_table: String,
        target_table: String,
        available_tables: Vec<String>,
        report: SchemaDiffReport,
        options: SchemaDiffOptions,
        migration_script: MigrationScript,
        language: AppLanguage,
    ) -> Self {
        Self {
            source_table,
            target_table,
            available_tables,
            report,
            options,
            migration_script,
            filter_tab: DiffFilterTab::DifferencesOnly,
            copied: false,
            language,
            on_select_source_table: None,
            on_select_target_table: None,
            on_swap_tables: None,
            on_change_filter: None,
            on_toggle_safe_mode: None,
            on_toggle_wrap_tx: None,
            on_change_direction: None,
            on_copy_sql: None,
            on_open_in_console: None,
            on_close: None,
        }
    }

    pub fn filter_tab(mut self, tab: DiffFilterTab) -> Self {
        self.filter_tab = tab;
        self
    }

    pub fn copied(mut self, copied: bool) -> Self {
        self.copied = copied;
        self
    }

    pub fn on_select_source_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_source_table = Some(Rc::new(handler));
        self
    }

    pub fn on_select_target_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_target_table = Some(Rc::new(handler));
        self
    }

    pub fn on_swap_tables<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_swap_tables = Some(Rc::new(handler));
        self
    }

    pub fn on_change_filter<F>(mut self, handler: F) -> Self
    where
        F: Fn(DiffFilterTab, &mut Window, &mut App) + 'static,
    {
        self.on_change_filter = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_safe_mode<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_safe_mode = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_wrap_tx<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_wrap_tx = Some(Rc::new(handler));
        self
    }

    pub fn on_change_direction<F>(mut self, handler: F) -> Self
    where
        F: Fn(MigrationDirection, &mut Window, &mut App) + 'static,
    {
        self.on_change_direction = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_sql<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_sql = Some(Rc::new(handler));
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

    // --- Sub-renderers ---

    fn render_header(&self) -> impl IntoElement {
        let lang = self.language;
        let on_close = self.on_close.clone();

        h_flex()
            .w_full()
            .px_5()
            .py_3p5()
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
                        Icon::new(IconName::GitCompare)
                            .size(px(20.0))
                            .text_color(ThemeColors::PRIMARY),
                    )
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_base()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("schema_diff.title", lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("schema_diff.subtitle", lang)),
                            ),
                    ),
            )
            .child(
                Button::new("schema_diff_close_btn")
                    .small()
                    .ghost()
                    .icon(IconName::Close)
                    .when_some(on_close, |btn, h| {
                        btn.on_click(move |_, window, cx| h(window, cx))
                    }),
            )
    }

    fn render_table_selectors_bar(&self) -> impl IntoElement {
        let lang = self.language;
        let tables = self.available_tables.clone();
        let src_tbl = self.source_table.clone();
        let tgt_tbl = self.target_table.clone();

        let on_src = self.on_select_source_table.clone();
        let on_tgt = self.on_select_target_table.clone();
        let on_swap = self.on_swap_tables.clone();

        // Source dropdown
        let src_tables_clone = tables.clone();
        let src_menu_btn = Button::new("diff_source_table_dropdown")
            .small()
            .outline()
            .icon(IconName::Database)
            .child(src_tbl.clone())
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for tbl in &src_tables_clone {
                    let name = tbl.clone();
                    let cb = on_src.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = cb {
                                handler(name.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        // Target dropdown
        let tgt_tables_clone = tables.clone();
        let tgt_menu_btn = Button::new("diff_target_table_dropdown")
            .small()
            .outline()
            .icon(IconName::Database)
            .child(tgt_tbl.clone())
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                for tbl in &tgt_tables_clone {
                    let name = tbl.clone();
                    let cb = on_tgt.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                        move |_, window, cx| {
                            if let Some(ref handler) = cb {
                                handler(name.clone(), window, cx);
                            }
                        },
                    ));
                }
                menu
            });

        // Swap button
        let swap_btn = Button::new("diff_swap_tables_btn")
            .small()
            .ghost()
            .icon(IconName::ArrowRightLeft)
            .tooltip(t("schema_diff.swap_tables", lang))
            .when_some(on_swap, |btn, h| {
                btn.on_click(move |_, window, cx| h(window, cx))
            });

        // Summary badges
        let summary = &self.report.summary;
        let add_count = summary.added_columns + summary.added_indexes;
        let mod_count = summary.modified_columns + summary.modified_indexes;
        let drop_count = summary.dropped_columns + summary.dropped_indexes;
        let same_count = summary.unchanged_columns + summary.unchanged_indexes;

        h_flex()
            .w_full()
            .px_5()
            .py_2p5()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!("{}:", t("schema_diff.source_table", lang))),
                            )
                            .child(src_menu_btn),
                    )
                    .child(swap_btn)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(format!("{}:", t("schema_diff.target_table", lang))),
                            )
                            .child(tgt_menu_btn),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .when(add_count > 0, |row| {
                        row.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(rgba(0x10b98120))
                                .text_color(rgba(0x34d399ff))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("+{} Add", add_count)),
                        )
                    })
                    .when(mod_count > 0, |row| {
                        row.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(rgba(0xf59e0b20))
                                .text_color(rgba(0xfbbf24ff))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("~{} Mod", mod_count)),
                        )
                    })
                    .when(drop_count > 0, |row| {
                        row.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(rgba(0xef444420))
                                .text_color(rgba(0xf87171ff))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("-{} Drop", drop_count)),
                        )
                    })
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .text_xs()
                            .child(format!("={} Same", same_count)),
                    ),
            )
    }

    fn render_filter_tabs(&self) -> impl IntoElement {
        let lang = self.language;
        let tabs = [
            (
                DiffFilterTab::DifferencesOnly,
                t("schema_diff.tab_differences_only", lang),
            ),
            (DiffFilterTab::All, t("schema_diff.tab_all", lang)),
            (
                DiffFilterTab::ColumnsOnly,
                t("schema_diff.tab_columns_only", lang),
            ),
            (
                DiffFilterTab::IndexesOnly,
                t("schema_diff.tab_indexes_only", lang),
            ),
        ];

        let mut row = h_flex()
            .items_center()
            .gap_1p5()
            .px_5()
            .py_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER);

        for (tab, label) in tabs {
            let is_sel = self.filter_tab == tab;
            let on_change = self.on_change_filter.clone();
            let pill = div()
                .id(ElementId::Name(format!("diff_tab_{:?}", tab).into()))
                .px_2p5()
                .py_1()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .bg(if is_sel {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BG_SURFACE_ACTIVE
                })
                .text_color(if is_sel {
                    ThemeColors::PRIMARY
                } else {
                    ThemeColors::TEXT_MUTED
                })
                .font_weight(if is_sel {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .hover(|s| {
                    s.bg(ThemeColors::PRIMARY_BORDER)
                        .text_color(ThemeColors::PRIMARY)
                })
                .child(label)
                .when_some(on_change, |el, h| {
                    el.on_click(move |_, window, cx| h(tab, window, cx))
                });

            row = row.child(pill);
        }

        row
    }

    fn render_comparison_matrix(&self) -> impl IntoElement {
        let mut list = v_flex().gap_2().p_4();

        // Filter columns
        let show_cols = self.filter_tab != DiffFilterTab::IndexesOnly;
        let show_indexes = self.filter_tab != DiffFilterTab::ColumnsOnly;
        let diffs_only = self.filter_tab == DiffFilterTab::DifferencesOnly;

        let filtered_cols: Vec<&ColumnDiff> = self
            .report
            .column_diffs
            .iter()
            .filter(|c| !diffs_only || c.status.is_different())
            .collect();

        let filtered_indexes: Vec<&IndexDiff> = self
            .report
            .index_diffs
            .iter()
            .filter(|i| !diffs_only || i.status.is_different())
            .collect();

        if (show_cols && filtered_cols.is_empty()) && (show_indexes && filtered_indexes.is_empty())
        {
            return div()
                .id("schema_diff_empty")
                .flex_1()
                .items_center()
                .justify_center()
                .p_8()
                .child(
                    div()
                        .text_sm()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(t("schema_diff.no_discrepancies", self.language)),
                );
        }

        // Render Columns
        if show_cols {
            for col in filtered_cols {
                list = list.child(self.render_column_diff_row(col));
            }
        }

        // Render Indexes
        if show_indexes {
            for idx in filtered_indexes {
                list = list.child(self.render_index_diff_row(idx));
            }
        }

        div()
            .id("schema_diff_cards_scroll")
            .flex_1()
            .max_h(px(250.0))
            .overflow_y_scroll()
            .child(list)
    }

    fn render_column_diff_row(&self, col: &ColumnDiff) -> impl IntoElement {
        let status_badge = match col.status {
            ColumnDiffStatus::Added => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0x10b98120))
                .text_color(rgba(0x34d399ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("ADD"),
            ColumnDiffStatus::Dropped => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0xef444420))
                .text_color(rgba(0xf87171ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("DROP"),
            ColumnDiffStatus::Modified { .. } => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0xf59e0b20))
                .text_color(rgba(0xfbbf24ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("MODIFY"),
            ColumnDiffStatus::Unchanged => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                .text_color(ThemeColors::TEXT_MUTED)
                .text_xs()
                .child("SAME"),
        };

        let src_desc = col
            .source
            .as_ref()
            .map(|c| {
                format!(
                    "{} {}{}",
                    c.data_type,
                    if c.is_nullable { "NULL" } else { "NOT NULL" },
                    if c.is_primary_key { " (PK)" } else { "" }
                )
            })
            .unwrap_or_else(|| "<missing>".to_string());

        let tgt_desc = col
            .target
            .as_ref()
            .map(|c| {
                format!(
                    "{} {}{}",
                    c.data_type,
                    if c.is_nullable { "NULL" } else { "NOT NULL" },
                    if c.is_primary_key { " (PK)" } else { "" }
                )
            })
            .unwrap_or_else(|| "<missing>".to_string());

        v_flex()
            .w_full()
            .p_2p5()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .gap_1p5()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex().items_center().gap_2().child(status_badge).child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_xs()
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(format!("COLUMN {}", col.name)),
                        ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(src_desc),
                            )
                            .child(
                                Icon::new(IconName::ArrowRight)
                                    .size(px(12.0))
                                    .text_color(ThemeColors::TEXT_FAINT),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(tgt_desc),
                            ),
                    ),
            )
            .when(!col.details.is_empty(), |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(col.details.join(" | ")),
                )
            })
    }

    fn render_index_diff_row(&self, idx: &IndexDiff) -> impl IntoElement {
        let status_badge = match idx.status {
            IndexDiffStatus::Added => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0x10b98120))
                .text_color(rgba(0x34d399ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("ADD"),
            IndexDiffStatus::Dropped => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0xef444420))
                .text_color(rgba(0xf87171ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("DROP"),
            IndexDiffStatus::Modified => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(rgba(0xf59e0b20))
                .text_color(rgba(0xfbbf24ff))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child("MODIFY"),
            IndexDiffStatus::Unchanged => div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                .text_color(ThemeColors::TEXT_MUTED)
                .text_xs()
                .child("SAME"),
        };

        v_flex()
            .w_full()
            .p_2p5()
            .rounded_md()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .gap_1p5()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex().items_center().gap_2().child(status_badge).child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_xs()
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text_color(rgba(0xa78bfafe))
                                .child(format!("INDEX {}", idx.name)),
                        ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(idx.details.join(" | ")),
                    ),
            )
    }

    fn render_migration_preview_box(&self) -> impl IntoElement {
        let lang = self.language;
        let script = &self.migration_script.full_script;

        let on_safe_toggle = self.on_toggle_safe_mode.clone();
        let cur_safe = !self.options.include_drops;

        let on_tx_toggle = self.on_toggle_wrap_tx.clone();
        let cur_tx = self.options.wrap_transaction;

        let on_dir_change = self.on_change_direction.clone();
        let cur_dir = self.options.direction;

        v_flex()
            .w_full()
            .p_4()
            .gap_2()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                // Options toolbar
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            // Direction switch
                            .child(
                                div()
                                    .id("diff_dir_switch")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(ThemeColors::BG_APP)
                                    .border_1()
                                    .border_color(ThemeColors::BORDER)
                                    .text_xs()
                                    .cursor_pointer()
                                    .child(match cur_dir {
                                        MigrationDirection::SourceToTarget => {
                                            format!("{} ➔ {}", self.source_table, self.target_table)
                                        }
                                        MigrationDirection::TargetToSource => {
                                            format!("{} ➔ {}", self.target_table, self.source_table)
                                        }
                                    })
                                    .when_some(on_dir_change, |el, h| {
                                        let next = match cur_dir {
                                            MigrationDirection::SourceToTarget => {
                                                MigrationDirection::TargetToSource
                                            }
                                            MigrationDirection::TargetToSource => {
                                                MigrationDirection::SourceToTarget
                                            }
                                        };
                                        el.on_click(move |_, window, cx| h(next, window, cx))
                                    }),
                            )
                            // Safe mode toggle
                            .child(
                                div()
                                    .id("diff_safe_mode_pill")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(if cur_safe {
                                        rgba(0x10b98120)
                                    } else {
                                        rgba(0xef444420)
                                    })
                                    .text_color(if cur_safe {
                                        rgba(0x34d399ff)
                                    } else {
                                        rgba(0xf87171ff)
                                    })
                                    .child(if cur_safe {
                                        t("schema_diff.safe_mode_on", lang)
                                    } else {
                                        t("schema_diff.safe_mode_off", lang)
                                    })
                                    .when_some(on_safe_toggle, |el, h| {
                                        el.on_click(move |_, window, cx| h(!cur_safe, window, cx))
                                    }),
                            )
                            // Transaction toggle
                            .child(
                                div()
                                    .id("diff_tx_toggle_pill")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(if cur_tx {
                                        ThemeColors::PRIMARY_BORDER
                                    } else {
                                        ThemeColors::BG_SURFACE_ACTIVE
                                    })
                                    .text_color(if cur_tx {
                                        ThemeColors::PRIMARY
                                    } else {
                                        ThemeColors::TEXT_MUTED
                                    })
                                    .child(t("schema_diff.wrap_transaction", lang))
                                    .when_some(on_tx_toggle, |el, h| {
                                        el.on_click(move |_, window, cx| h(!cur_tx, window, cx))
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!(
                                "{} {} {}",
                                self.migration_script.statements.len(),
                                t("schema_diff.statements", lang),
                                match self.report.family {
                                    crate::db::types::DatabaseFamily::Postgres => "(PostgreSQL)",
                                    crate::db::types::DatabaseFamily::MySql => "(MySQL)",
                                    crate::db::types::DatabaseFamily::Sqlite => "(SQLite)",
                                }
                            )),
                    ),
            )
            .child(
                div()
                    .id("schema_diff_sql_preview")
                    .w_full()
                    .max_h(px(140.0))
                    .overflow_y_scroll()
                    .p_3()
                    .rounded_md()
                    .bg(ThemeColors::BG_APP)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_xs()
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(script.clone()),
            )
    }

    fn render_footer(&self) -> impl IntoElement {
        let lang = self.language;
        let script = self.migration_script.full_script.clone();
        let on_copy = self.on_copy_sql.clone();
        let on_console = self.on_open_in_console.clone();
        let on_close = self.on_close.clone();
        let is_copied = self.copied;

        let script_for_copy = script.clone();
        let script_for_console = script.clone();

        h_flex()
            .w_full()
            .px_5()
            .py_3()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            .child(
                Button::new("schema_diff_cancel_btn")
                    .ghost()
                    .label(t("common.close", lang))
                    .when_some(on_close, |btn, h| {
                        btn.on_click(move |_, window, cx| h(window, cx))
                    }),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("schema_diff_copy_btn")
                            .outline()
                            .icon(if is_copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .label(if is_copied {
                                t("schema_diff.copied", lang)
                            } else {
                                t("schema_diff.copy_sql", lang)
                            })
                            .when_some(on_copy, |btn, h| {
                                btn.on_click(move |_, window, cx| {
                                    h(script_for_copy.clone(), window, cx)
                                })
                            }),
                    )
                    .child(
                        Button::new("schema_diff_open_console_btn")
                            .primary()
                            .icon(IconName::SquareArrowOutUpRight)
                            .label(t("schema_diff.open_in_console", lang))
                            .when_some(on_console, |btn, h| {
                                btn.on_click(move |_, window, cx| {
                                    h(script_for_console.clone(), window, cx)
                                })
                            }),
                    ),
            )
    }
}

impl RenderOnce for SchemaDiffModal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let dialog = v_flex()
            .w(px(760.0))
            .max_h(px(640.0))
            .rounded_lg()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .shadow_lg()
            .overflow_hidden()
            .child(self.render_header())
            .child(self.render_table_selectors_bar())
            .child(self.render_filter_tabs())
            .child(self.render_comparison_matrix())
            .child(self.render_migration_preview_box())
            .child(self.render_footer());

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000088))
            .items_center()
            .justify_center()
            .child(dialog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::{ColumnInfo, DatabaseFamily};

    #[test]
    fn test_schema_diff_modal_creation() {
        let report = SchemaDiffReport {
            source_table: "t1".to_string(),
            source_schema: None,
            target_table: "t2".to_string(),
            target_schema: None,
            family: DatabaseFamily::Postgres,
            column_diffs: vec![ColumnDiff {
                name: "col1".to_string(),
                status: ColumnDiffStatus::Added,
                source: Some(ColumnInfo {
                    name: "col1".to_string(),
                    data_type: "INTEGER".to_string(),
                    is_nullable: true,
                    is_primary_key: false,
                    is_auto_increment: false,
                    default_value: None,
                    description: None,
                }),
                target: None,
                details: vec!["Added".to_string()],
            }],
            index_diffs: vec![],
            summary: Default::default(),
        };

        let modal = SchemaDiffModal::new(
            "t1".to_string(),
            "t2".to_string(),
            vec!["t1".to_string(), "t2".to_string()],
            report,
            SchemaDiffOptions::default(),
            MigrationScript::default(),
            AppLanguage::En,
        );

        assert_eq!(modal.filter_tab, DiffFilterTab::DifferencesOnly);
        assert_eq!(modal.source_table, "t1");
        assert_eq!(modal.target_table, "t2");
    }
}
