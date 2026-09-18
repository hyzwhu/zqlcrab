//! Sidebar navigation displaying saved connection profiles and database schema tree.

use crate::db::types::{ConnectionConfig, DatabaseType, TableInfo};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::gpui::{
    App, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct Sidebar {
    connections: Vec<ConnectionConfig>,
    active_connection_id: Option<String>,
    active_tables: Vec<TableInfo>,
    table_filter: String,
    selected_table: Option<String>,
    on_new_connection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_select_connection: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_table: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_disconnect: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_refresh: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_quick_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
}

impl Sidebar {
    pub fn new(
        connections: Vec<ConnectionConfig>,
        active_connection_id: Option<String>,
        active_tables: Vec<TableInfo>,
    ) -> Self {
        Self {
            connections,
            active_connection_id,
            active_tables,
            table_filter: String::new(),
            selected_table: None,
            on_new_connection: None,
            on_select_connection: None,
            on_select_table: None,
            on_disconnect: None,
            on_refresh: None,
            on_quick_query: None,
        }
    }

    pub fn table_filter(mut self, filter: impl Into<String>) -> Self {
        self.table_filter = filter.into();
        self
    }

    pub fn selected_table(mut self, table: Option<String>) -> Self {
        self.selected_table = table;
        self
    }

    pub fn on_new_connection<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_new_connection = Some(Rc::new(handler));
        self
    }

    pub fn on_select_connection<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_connection = Some(Rc::new(handler));
        self
    }

    pub fn on_select_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_table = Some(Rc::new(handler));
        self
    }

    pub fn on_disconnect<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_disconnect = Some(Rc::new(handler));
        self
    }

    pub fn on_refresh<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh = Some(Rc::new(handler));
        self
    }

    pub fn on_quick_query<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_quick_query = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let on_new_action = self.on_new_connection.clone();
        let mut new_btn = Button::new("new_conn")
            .primary()
            .xsmall()
            .icon(IconName::Plus)
            .label("New")
            .tooltip("New Connection Profile");

        if let Some(ref on_new) = on_new_action {
            let on_new = on_new.clone();
            new_btn = new_btn.on_click(move |_, window, cx| {
                on_new(window, cx);
            });
        }

        let mut refresh_btn = Button::new("refresh_sidebar")
            .ghost()
            .xsmall()
            .icon(IconName::RotateCw)
            .tooltip("Refresh Tables");

        if let Some(on_refresh) = self.on_refresh {
            refresh_btn = refresh_btn.on_click(move |_, window, cx| {
                on_refresh(window, cx);
            });
        }

        let mut disconnect_btn = Button::new("disconnect_sidebar")
            .ghost()
            .xsmall()
            .icon(IconName::Unplug)
            .tooltip("Disconnect");

        if let Some(on_disconnect) = self.on_disconnect {
            disconnect_btn = disconnect_btn.on_click(move |_, window, cx| {
                on_disconnect(window, cx);
            });
        }

        let header = h_flex()
            .h(px(40.0))
            .w_full()
            .px_3()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Icon::new(IconName::Database)
                            .size(px(14.0))
                            .text_color(ThemeColors::PRIMARY_BORDER),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("CONNECTIONS"),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .when(self.active_connection_id.is_some(), |this| {
                        this.child(refresh_btn).child(disconnect_btn)
                    })
                    .child(new_btn),
            );

        // Connections list
        let mut conn_list = v_flex()
            .id("sidebar_conn_scroll")
            .gap_1()
            .p_2();

        for conn in &self.connections {
            let is_active = self.active_connection_id.as_deref() == Some(&conn.id);
            let conn_id = conn.id.clone();
            let on_select = self.on_select_connection.clone();

            let engine_icon = match conn.db_type {
                DatabaseType::Sqlite => IconName::Database,
                DatabaseType::Mysql => IconName::Cpu,
                DatabaseType::MariaDB => IconName::Server,
                DatabaseType::TiDB => IconName::Layers,
                DatabaseType::OceanBase => IconName::HardDrive,
                DatabaseType::StarRocks | DatabaseType::Doris => IconName::Activity,
                DatabaseType::PolarDB => IconName::Server,
                DatabaseType::Postgres => IconName::Layers,
                DatabaseType::CockroachDB => IconName::Cpu,
                DatabaseType::TimescaleDB => IconName::Activity,
                DatabaseType::Redshift => IconName::Layers,
                DatabaseType::YugabyteDB => IconName::HardDrive,
                DatabaseType::OpenGauss | DatabaseType::Kingbase => IconName::Server,
                DatabaseType::Greenplum => IconName::Layers,
            };

            let conn_item = h_flex()
                .id(ElementId::Name(format!("conn_item_{}", conn.id).into()))
                .w_full()
                .p_2()
                .rounded_md()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .bg(if is_active {
                    ThemeColors::BG_SURFACE_ACTIVE
                } else {
                    ThemeColors::BG_SURFACE
                })
                .hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER))
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(6.0))
                                .h(px(6.0))
                                .rounded_full()
                                .bg(if is_active {
                                    ThemeColors::SUCCESS
                                } else {
                                    ThemeColors::TEXT_FAINT
                                }),
                        )
                        .child(
                            Icon::new(engine_icon)
                                .size(px(14.0))
                                .text_color(if is_active {
                                    ThemeColors::PRIMARY_BORDER
                                } else {
                                    ThemeColors::TEXT_MUTED
                                }),
                        )
                        .child(
                            v_flex()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(ThemeColors::TEXT_PRIMARY)
                                                .child(conn.name.clone()),
                                        )
                                        .when(conn.environment.is_production(), |this| {
                                            this.child(
                                                div()
                                                    .px_1()
                                                    .rounded_sm()
                                                    .bg(ThemeColors::ERROR)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("PROD"),
                                            )
                                        })
                                        .when(conn.is_read_only, |this| {
                                            this.child(
                                                div()
                                                    .px_1()
                                                    .rounded_sm()
                                                    .bg(ThemeColors::WARNING)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("RO"),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_FAINT)
                                        .child(format!("{}: {}", conn.db_type, conn.database)),
                                ),
                        ),
                )
                .on_click(move |_, window, cx| {
                    if let Some(ref handler) = on_select {
                        handler(conn_id.clone(), window, cx);
                    }
                });

            conn_list = conn_list.child(conn_item);
        }

        let mut add_conn_bottom = Button::new("add_conn_bottom")
            .outline()
            .xsmall()
            .w_full()
            .icon(IconName::Plus)
            .label("Add Connection");
        if let Some(ref on_new) = on_new_action {
            let on_new = on_new.clone();
            add_conn_bottom = add_conn_bottom.on_click(move |_, window, cx| {
                on_new(window, cx);
            });
        }
        conn_list = conn_list.child(add_conn_bottom);

        // Tables list when connected
        let filter = self.table_filter.trim().to_lowercase();
        let filtered_tables: Vec<&TableInfo> = if filter.is_empty() {
            self.active_tables.iter().collect()
        } else {
            self.active_tables
                .iter()
                .filter(|t| t.name.to_lowercase().contains(&filter))
                .collect()
        };

        let on_quick = self.on_quick_query.clone();

        let tables_pane = v_flex()
            .flex_1()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .child(
                h_flex()
                    .p_2()
                    .items_center()
                    .justify_between()
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(if filter.is_empty() {
                                format!("TABLES ({})", self.active_tables.len())
                            } else {
                                format!("TABLES ({}/{})", filtered_tables.len(), self.active_tables.len())
                            }),
                    ),
            )
            .child({
                let mut tbl_list = v_flex()
                    .id("sidebar_tables_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .gap_0p5()
                    .p_1();

                for (idx, tbl) in filtered_tables.iter().enumerate() {
                    let is_selected = self.selected_table.as_deref() == Some(&tbl.name);
                    let tbl_name = tbl.name.clone();
                    let on_tbl_select = self.on_select_table.clone();

                    let tbl_for_query = tbl.name.clone();
                    let tbl_for_count = tbl.name.clone();
                    let on_quick_select = on_quick.clone();
                    let on_quick_count = on_quick.clone();

                    let is_view = tbl.table_type == "VIEW";
                    let icon_name = if is_view {
                        IconName::FileText
                    } else {
                        IconName::Table
                    };

                    let row = h_flex()
                        .id(ElementId::NamedInteger("tbl_row".into(), idx as u64))
                        .w_full()
                        .py_1()
                        .px_2()
                        .rounded_sm()
                        .items_center()
                        .justify_between()
                        .cursor_pointer()
                        .bg(if is_selected {
                            ThemeColors::PRIMARY
                        } else {
                            ThemeColors::BG_APP
                        })
                        .hover(|s| {
                            if !is_selected {
                                s.bg(ThemeColors::BG_SURFACE_HOVER)
                            } else {
                                s
                            }
                        })
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(icon_name)
                                        .size(px(13.0))
                                        .text_color(if is_selected {
                                            ThemeColors::TEXT_PRIMARY
                                        } else {
                                            ThemeColors::PRIMARY_BORDER
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(ThemeColors::TEXT_PRIMARY)
                                        .child(tbl.name.clone()),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new(ElementId::NamedInteger("quick_sel".into(), idx as u64))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Play)
                                        .tooltip("SELECT * LIMIT 100")
                                        .when_some(on_quick_select, |btn, handler| {
                                            btn.on_click(move |_, window, cx| {
                                                handler(
                                                    format!("SELECT * FROM \"{}\" LIMIT 100;", tbl_for_query),
                                                    window,
                                                    cx,
                                                );
                                            })
                                        }),
                                )
                                .child(
                                    Button::new(ElementId::NamedInteger("quick_cnt".into(), idx as u64))
                                        .ghost()
                                        .xsmall()
                                        .tooltip("COUNT(*)")
                                        .child("#")
                                        .when_some(on_quick_count, |btn, handler| {
                                            btn.on_click(move |_, window, cx| {
                                                handler(
                                                    format!("SELECT COUNT(*) AS total_count FROM \"{}\";", tbl_for_count),
                                                    window,
                                                    cx,
                                                );
                                            })
                                        }),
                                ),
                        )
                        .on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_tbl_select {
                                handler(tbl_name.clone(), window, cx);
                            }
                        });

                    tbl_list = tbl_list.child(row);
                }
                tbl_list
            });

        v_flex()
            .w(px(260.0))
            .h_full()
            .bg(ThemeColors::BG_APP)
            .border_r_1()
            .border_color(ThemeColors::BORDER)
            .child(header)
            .child(conn_list)
            .when(self.active_connection_id.is_some(), |this| {
                this.child(tables_pane)
            })
    }
}
