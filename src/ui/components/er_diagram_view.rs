//! Visual Entity-Relationship (ER) Diagram canvas and table relationship graph view.
//!
//! Renders interactive table cards displaying schema definitions, primary keys,
//! foreign keys, and visual relationship connectors across the active database schema.

use crate::db::er_diagram::{ErDiagramGraph, TableNode};
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
};
use gpui_kit::gpui::{
    App, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
    px,
};
use std::collections::HashSet;
use std::rc::Rc;

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct ErDiagramView {
    graph: Option<ErDiagramGraph>,
    is_loading: bool,
    search_input: Entity<InputState>,
    selected_table: Option<String>,
    zoom: f32,
    mermaid_copied: bool,
    language: AppLanguage,
    on_select_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_zoom_in: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_zoom_out: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_zoom_reset: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_copy_mermaid: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_view_data: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_view_schema: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_refresh: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl ErDiagramView {
    pub fn new(
        graph: Option<ErDiagramGraph>,
        is_loading: bool,
        search_input: Entity<InputState>,
        selected_table: Option<String>,
        zoom: f32,
        mermaid_copied: bool,
        language: AppLanguage,
    ) -> Self {
        Self {
            graph,
            is_loading,
            search_input,
            selected_table,
            zoom,
            mermaid_copied,
            language,
            on_select_table: None,
            on_zoom_in: None,
            on_zoom_out: None,
            on_zoom_reset: None,
            on_copy_mermaid: None,
            on_view_data: None,
            on_view_schema: None,
            on_refresh: None,
        }
    }

    pub fn on_select_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_table = Some(Rc::new(handler));
        self
    }

    pub fn on_zoom_in<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_zoom_in = Some(Rc::new(handler));
        self
    }

    pub fn on_zoom_out<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_zoom_out = Some(Rc::new(handler));
        self
    }

    pub fn on_zoom_reset<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_zoom_reset = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_mermaid<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_mermaid = Some(Rc::new(handler));
        self
    }

    pub fn on_view_data<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_view_data = Some(Rc::new(handler));
        self
    }

    pub fn on_view_schema<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_view_schema = Some(Rc::new(handler));
        self
    }

    pub fn on_refresh<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh = Some(Rc::new(handler));
        self
    }

    fn render_header(&self, _window: &Window, _cx: &App) -> impl IntoElement {
        let total_tables = self.graph.as_ref().map(|g| g.tables.len()).unwrap_or(0);
        let total_relations = self.graph.as_ref().map(|g| g.relations.len()).unwrap_or(0);

        let lang = self.language;

        // Statistics Badges
        let stats_badge = h_flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(format!("{total_tables} {}", t("er_diagram.tables", lang))),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .bg(ThemeColors::PRIMARY_BG)
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(ThemeColors::PRIMARY_LIGHT)
                    .child(format!(
                        "{total_relations} {}",
                        t("er_diagram.relations", lang)
                    )),
            );

        // Zoom Controls
        let zoom_pct = (self.zoom * 100.0).round() as u32;
        let on_zin = self.on_zoom_in.clone();
        let on_zout = self.on_zoom_out.clone();
        let on_zreset = self.on_zoom_reset.clone();

        let mut zoom_out_btn = Button::new("er_zoom_out")
            .ghost()
            .xsmall()
            .icon(IconName::Minus)
            .tooltip("Zoom Out");
        if let Some(handler) = on_zout {
            zoom_out_btn = zoom_out_btn.on_click(move |_, window, cx| handler(window, cx));
        }

        let mut zoom_reset_btn = Button::new("er_zoom_reset")
            .ghost()
            .xsmall()
            .child(format!("{zoom_pct}%"))
            .tooltip("Reset Zoom (100%)");
        if let Some(handler) = on_zreset {
            zoom_reset_btn = zoom_reset_btn.on_click(move |_, window, cx| handler(window, cx));
        }

        let mut zoom_in_btn = Button::new("er_zoom_in")
            .ghost()
            .xsmall()
            .icon(IconName::Plus)
            .tooltip("Zoom In");
        if let Some(handler) = on_zin {
            zoom_in_btn = zoom_in_btn.on_click(move |_, window, cx| handler(window, cx));
        }

        let zoom_bar = h_flex()
            .items_center()
            .gap_0p5()
            .bg(ThemeColors::BG_SURFACE)
            .p_0p5()
            .rounded_md()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .child(zoom_out_btn)
            .child(zoom_reset_btn)
            .child(zoom_in_btn);

        // Mermaid Export Button
        let on_mermaid = self.on_copy_mermaid.clone();
        let mermaid_code = self
            .graph
            .as_ref()
            .map(|g| g.to_mermaid())
            .unwrap_or_default();
        let mut mermaid_btn = Button::new("er_export_mermaid")
            .ghost()
            .xsmall()
            .icon(if self.mermaid_copied {
                IconName::Check
            } else {
                IconName::Copy
            })
            .child(if self.mermaid_copied {
                t("er_diagram.copied", lang)
            } else {
                t("er_diagram.export_mermaid", lang)
            })
            .tooltip("Export and copy diagram in Mermaid markdown syntax");

        if let Some(handler) = on_mermaid {
            let code = mermaid_code.clone();
            mermaid_btn = mermaid_btn.on_click(move |_, window, cx| {
                handler(code.clone(), window, cx);
            });
        }

        // Refresh Button
        let on_ref = self.on_refresh.clone();
        let mut refresh_btn = Button::new("er_refresh_btn")
            .ghost()
            .xsmall()
            .icon(IconName::RotateCcw)
            .tooltip("Reload tables and foreign key relationships");
        if let Some(handler) = on_ref {
            refresh_btn = refresh_btn.on_click(move |_, window, cx| handler(window, cx));
        }

        h_flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Workflow)
                                    .size(px(18.0))
                                    .text_color(ThemeColors::PRIMARY_BORDER),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("workspace.er_diagram", lang)),
                            ),
                    )
                    .child(stats_badge)
                    .child(
                        div()
                            .w(px(200.0))
                            .child(Input::new(&self.search_input).xsmall()),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(zoom_bar)
                    .child(mermaid_btn)
                    .child(refresh_btn),
            )
    }

    fn render_table_card(
        &self,
        node: &TableNode,
        is_selected: bool,
        is_connected: bool,
        is_faded: bool,
    ) -> impl IntoElement {
        let tbl_for_sel = node.name.clone();
        let tbl_for_data = node.name.clone();
        let tbl_for_schema = node.name.clone();

        let on_sel = self.on_select_table.clone();
        let on_data = self.on_view_data.clone();
        let on_schema = self.on_view_schema.clone();

        let lang = self.language;

        // Border and glow state
        let (border_col, bg_col) = if is_selected {
            (ThemeColors::PRIMARY_BORDER, ThemeColors::BG_SURFACE)
        } else if is_connected {
            (ThemeColors::SUCCESS, ThemeColors::BG_SURFACE)
        } else {
            (ThemeColors::BORDER, ThemeColors::BG_SURFACE)
        };

        // Header
        let header =
            h_flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .bg(if is_selected {
                    ThemeColors::PRIMARY_BG
                } else {
                    ThemeColors::BG_SURFACE_ACTIVE
                })
                .border_b_1()
                .border_color(border_col)
                .child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .child(Icon::new(IconName::Table).size(px(14.0)).text_color(
                            if is_selected {
                                ThemeColors::PRIMARY_LIGHT
                            } else {
                                ThemeColors::TEXT_MUTED
                            },
                        ))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_selected {
                                    ThemeColors::PRIMARY_LIGHT
                                } else {
                                    ThemeColors::TEXT_PRIMARY
                                })
                                .child(node.name.clone()),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .when(node.foreign_keys.len() > 0, |this| {
                            this.child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::PRIMARY_BG)
                                    .text_color(ThemeColors::PRIMARY_LIGHT)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("{} FK", node.foreign_keys.len())),
                            )
                        })
                        .when_some(node.row_count_estimate, |this, count| {
                            this.child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child(format!("~{count}")),
                            )
                        }),
                );

        // Column List (up to 12 items displayed)
        let mut col_list = v_flex().w_full().p_1().gap_0p5();
        let max_display = 12;
        for col in node.columns.iter().take(max_display) {
            let is_pk = col.is_primary_key;
            let is_fk = node.is_foreign_key(&col.name);

            let key_badge = if is_pk {
                div()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .bg(ThemeColors::WARNING)
                    .text_color(ThemeColors::BG_APP)
                    .text_size(px(9.0))
                    .font_weight(FontWeight::BOLD)
                    .child("PK")
            } else if is_fk {
                div()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .bg(ThemeColors::PRIMARY)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .text_size(px(9.0))
                    .font_weight(FontWeight::BOLD)
                    .child("FK")
            } else {
                div()
                    .w(px(14.0))
                    .text_center()
                    .text_size(px(10.0))
                    .text_color(ThemeColors::TEXT_FAINT)
                    .child("•")
            };

            let col_name_elem = div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .font_weight(if is_pk || is_fk {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_pk {
                    ThemeColors::TEXT_PRIMARY
                } else if is_fk {
                    ThemeColors::PRIMARY_LIGHT
                } else {
                    ThemeColors::TEXT_MUTED
                })
                .child(col.name.clone());

            let col_type_elem = div()
                .text_size(px(10.0))
                .font_family("JetBrains Mono")
                .text_color(ThemeColors::TEXT_FAINT)
                .child(col.data_type.clone());

            let col_row = h_flex()
                .items_center()
                .justify_between()
                .px_2()
                .py_1()
                .rounded_sm()
                .gap_1p5()
                .hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER))
                .child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .flex_1()
                        .min_w_0()
                        .child(key_badge)
                        .child(col_name_elem),
                )
                .child(col_type_elem);

            col_list = col_list.child(col_row);
        }

        if node.columns.len() > max_display {
            let rem = node.columns.len() - max_display;
            col_list = col_list.child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.0))
                    .text_center()
                    .text_color(ThemeColors::TEXT_FAINT)
                    .child(format!("+ {rem} more columns...")),
            );
        }

        // Footer Actions
        let mut data_btn = Button::new(ElementId::NamedInteger(
            "er_data_btn".into(),
            node.name.len() as u64,
        ))
        .ghost()
        .xsmall()
        .icon(IconName::Play)
        .child(t("er_diagram.view_data", lang))
        .tooltip("Open Table Data in DataGrid");
        if let Some(handler) = on_data {
            let tname = tbl_for_data.clone();
            data_btn = data_btn.on_click(move |_, window, cx| handler(tname.clone(), window, cx));
        }

        let mut schema_btn = Button::new(ElementId::NamedInteger(
            "er_schema_btn".into(),
            node.name.len() as u64,
        ))
        .ghost()
        .xsmall()
        .icon(IconName::TableProperties)
        .child(t("er_diagram.view_schema", lang))
        .tooltip("Inspect Table Schema");
        if let Some(handler) = on_schema {
            let tname = tbl_for_schema.clone();
            schema_btn =
                schema_btn.on_click(move |_, window, cx| handler(tname.clone(), window, cx));
        }

        let footer = h_flex()
            .items_center()
            .justify_end()
            .gap_1()
            .px_2()
            .py_1()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(data_btn)
            .child(schema_btn);

        // Entire Card Container
        let mut card = v_flex()
            .id(ElementId::NamedInteger(
                "er_table_card".into(),
                node.name.len() as u64,
            ))
            .w(px(node.width))
            .rounded_md()
            .bg(bg_col)
            .border_1()
            .border_color(border_col)
            .overflow_hidden()
            .shadow_sm()
            .when(is_faded, |s| s.opacity(0.45))
            .child(header)
            .child(col_list)
            .child(footer);

        if let Some(handler) = on_sel {
            let tname = tbl_for_sel.clone();
            card = card.on_click(move |_, window, cx| {
                handler(tname.clone(), window, cx);
            });
        }

        card
    }

    fn render_relationships_bar(&self) -> impl IntoElement {
        let Some(graph) = &self.graph else {
            return div().into_any_element();
        };

        if graph.relations.is_empty() {
            return div().into_any_element();
        }

        let selected = self.selected_table.as_deref();
        let on_sel = self.on_select_table.clone();

        let mut chips = h_flex().items_center().gap_1p5().flex_wrap();
        for (idx, rel) in graph.relations.iter().enumerate() {
            let is_rel_focused = selected
                .map(|s| {
                    s.eq_ignore_ascii_case(&rel.from_table) || s.eq_ignore_ascii_case(&rel.to_table)
                })
                .unwrap_or(false);

            let chip_bg = if is_rel_focused {
                ThemeColors::PRIMARY_BG
            } else {
                ThemeColors::BG_SURFACE_ACTIVE
            };

            let chip_border = if is_rel_focused {
                ThemeColors::PRIMARY_BORDER
            } else {
                ThemeColors::BORDER
            };

            let target_table_for_click = rel.to_table.clone();
            let on_click_chip = on_sel.clone();

            let mut chip = h_flex()
                .id(ElementId::NamedInteger("rel_chip".into(), idx as u64))
                .items_center()
                .gap_1p5()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(chip_bg)
                .border_1()
                .border_color(chip_border)
                .cursor_pointer()
                .child(
                    Icon::new(IconName::Link)
                        .size(px(12.0))
                        .text_color(if is_rel_focused {
                            ThemeColors::PRIMARY_LIGHT
                        } else {
                            ThemeColors::TEXT_FAINT
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(format!("{}.{}", rel.from_table, rel.from_columns.join("_"))),
                )
                .child(
                    div()
                        .px_1()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::BG_SURFACE)
                        .text_size(px(9.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::PRIMARY_LIGHT)
                        .child(rel.cardinality.badge_label()),
                )
                .child(
                    Icon::new(IconName::ArrowRight)
                        .size(px(11.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::PRIMARY_BORDER)
                        .child(format!("{}.{}", rel.to_table, rel.to_columns.join("_"))),
                );

            if let Some(handler) = on_click_chip {
                chip = chip.on_click(move |_, window, cx| {
                    handler(target_table_for_click.clone(), window, cx);
                });
            }

            chips = chips.child(chip);
        }

        v_flex()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .gap_1()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::GitCompare)
                            .size(px(12.0))
                            .text_color(ThemeColors::TEXT_MUTED),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child("FOREIGN KEY RELATIONSHIPS"),
                    ),
            )
            .child(chips)
            .into_any_element()
    }
}

impl RenderOnce for ErDiagramView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.is_loading {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .bg(ThemeColors::BG_APP)
                .child(
                    Icon::new(IconName::Workflow)
                        .size(px(40.0))
                        .text_color(ThemeColors::PRIMARY_BORDER),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("Generating ER Diagram & Mapping Relationships..."),
                )
                .into_any_element();
        }

        let Some(graph) = &self.graph else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .bg(ThemeColors::BG_APP)
                .child(
                    Icon::new(IconName::Database)
                        .size(px(44.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("No Database Selected"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("Connect to a database to inspect its entity-relationship diagram."),
                )
                .into_any_element();
        };

        if graph.tables.is_empty() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .bg(ThemeColors::BG_APP)
                .child(
                    Icon::new(IconName::TableProperties)
                        .size(px(44.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("No Tables in Schema"),
                )
                .child(div().text_xs().text_color(ThemeColors::TEXT_MUTED).child(
                    "Create tables or switch schemas to view table structure and relationships.",
                ))
                .into_any_element();
        }

        let header = self.render_header(window, cx);
        let rel_bar = self.render_relationships_bar();

        let filter_query = self
            .search_input
            .read(cx)
            .value()
            .to_string()
            .trim()
            .to_lowercase();
        let selected_table = self.selected_table.clone();

        let connected_set: HashSet<String> = if let Some(sel) = &selected_table {
            graph.connected_tables(sel)
        } else {
            HashSet::new()
        };

        // Render Canvas Grid
        let mut canvas_grid = h_flex().gap_6().flex_wrap().items_start().p_6();

        for tbl in &graph.tables {
            let matches_filter =
                filter_query.is_empty() || tbl.name.to_lowercase().contains(&filter_query);

            if !matches_filter {
                continue;
            }

            let is_sel = selected_table
                .as_ref()
                .map(|s| s.eq_ignore_ascii_case(&tbl.name))
                .unwrap_or(false);

            let is_conn = connected_set.contains(&tbl.name);

            let is_faded = selected_table.is_some() && !is_sel && !is_conn;

            let card = self.render_table_card(tbl, is_sel, is_conn, is_faded);
            canvas_grid = canvas_grid.child(card);
        }

        v_flex()
            .id("er_diagram_view_container")
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(header)
            .child(rel_bar)
            .child(
                div()
                    .id("er_diagram_canvas_scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .overflow_x_scroll()
                    .child(canvas_grid),
            )
            .into_any_element()
    }
}
