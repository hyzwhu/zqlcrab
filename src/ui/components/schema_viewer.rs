//! Schema structure viewer displaying column definitions, constraints, indexes, and DDL.

use crate::db::types::{ColumnInfo, IndexInfo};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
};
use gpui_kit::gpui::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::*, px,
};

#[derive(IntoElement)]
pub struct SchemaViewer {
    table_name: Option<String>,
    columns: Vec<ColumnInfo>,
    indexes: Vec<IndexInfo>,
    ddl: Option<String>,
}

impl SchemaViewer {
    pub fn new(
        table_name: Option<String>,
        columns: Vec<ColumnInfo>,
        indexes: Vec<IndexInfo>,
        ddl: Option<String>,
    ) -> Self {
        Self {
            table_name,
            columns,
            indexes,
            ddl,
        }
    }
}

impl RenderOnce for SchemaViewer {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Some(table_name) = self.table_name else {
            return v_flex()
                .id("schema_viewer_empty")
                .size_full()
                .justify_center()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::TableProperties)
                        .size(px(32.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("No Table Selected"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("Select a table from the sidebar to inspect its columns, keys, and DDL."),
                );
        };

        // Header
        let header = h_flex()
            .items_center()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                Icon::new(IconName::TableProperties)
                    .size(px(18.0))
                    .text_color(ThemeColors::PRIMARY_BORDER),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(format!("Table Structure: {table_name}")),
            );

        // Columns section
        let col_header_row = TableRow::new()
            .child(TableHead::new().child("Name"))
            .child(TableHead::new().child("Type"))
            .child(TableHead::new().child("Nullable"))
            .child(TableHead::new().child("Key"))
            .child(TableHead::new().child("Default"));

        let mut col_body = TableBody::new();
        for col in &self.columns {
            let row = TableRow::new()
                .child(
                    TableCell::new().child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(col.name.clone()),
                    ),
                )
                .child(
                    TableCell::new().child(
                        div()
                            .text_xs()
                            .font_family("JetBrains Mono")
                            .text_color(ThemeColors::PRIMARY_BORDER)
                            .child(col.data_type.clone()),
                    ),
                )
                .child(
                    TableCell::new().child(
                        div()
                            .text_xs()
                            .text_color(if col.is_nullable {
                                ThemeColors::TEXT_MUTED
                            } else {
                                ThemeColors::WARNING
                            })
                            .child(if col.is_nullable { "YES" } else { "NO" }),
                    ),
                )
                .child(
                    TableCell::new().child(
                        h_flex()
                            .gap_1()
                            .when(col.is_primary_key, |this| {
                                this.child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(ThemeColors::PRIMARY)
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                        .child("PK"),
                                )
                            })
                            .when(col.is_auto_increment, |this| {
                                this.child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_MUTED)
                                        .child("AUTO"),
                                )
                            }),
                    ),
                )
                .child(
                    TableCell::new().child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(col.default_value.clone().unwrap_or_else(|| "-".to_string())),
                    ),
                );
            col_body = col_body.child(row);
        }

        let columns_table = Table::new()
            .small()
            .child(TableHeader::new().child(col_header_row))
            .child(col_body);

        // Indexes section
        let indexes_section = v_flex()
            .gap_2()
            .p_3()
            .when(!self.indexes.is_empty(), |this| {
                let mut index_list = v_flex().gap_1();
                for idx in &self.indexes {
                    let pill = h_flex()
                        .items_center()
                        .gap_2()
                        .p_2()
                        .rounded_md()
                        .bg(ThemeColors::BG_SURFACE)
                        .child(
                            Icon::new(IconName::Key)
                                .size(px(14.0))
                                .text_color(ThemeColors::TEXT_MUTED),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(idx.name.clone()),
                        )
                        .when(idx.is_primary, |sub| {
                            sub.child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::PRIMARY)
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("Primary"),
                            )
                        })
                        .when(idx.is_unique, |sub| {
                            sub.child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::SUCCESS)
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("Unique"),
                            )
                        });
                    index_list = index_list.child(pill);
                }

                this.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("INDEXES"),
                )
                .child(index_list)
            });

        // DDL section
        let ddl_section = v_flex()
            .gap_2()
            .p_3()
            .when_some(self.ddl, |this, sql| {
                this.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("DDL STATEMENT"),
                )
                .child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(ThemeColors::BG_SURFACE)
                        .border_1()
                        .border_color(ThemeColors::BORDER)
                        .font_family("JetBrains Mono")
                        .text_xs()
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(sql),
                )
            });

        v_flex()
            .id("schema_viewer_scroll")
            .size_full()
            .bg(ThemeColors::BG_APP)
            .overflow_y_scroll()
            .child(header)
            .child(
                div()
                    .p_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child("COLUMNS"),
                    )
                    .child(columns_table),
            )
            .child(indexes_section)
            .child(ddl_section)
    }
}
