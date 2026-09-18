//! Sidebar navigation displaying saved connection profiles and database schema tree.

use crate::db::types::{ConnectionConfig, DatabaseFamily, DatabaseType, TableInfo};
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
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(IntoElement)]
pub struct Sidebar {
    connections: Vec<ConnectionConfig>,
    active_connection_id: Option<String>,
    active_tables: Vec<TableInfo>,
    table_filter: Entity<InputState>,
    selected_table: Option<String>,
    on_new_connection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_select_connection: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_table: Option<Rc<dyn Fn(TableInfo, &mut Window, &mut App) + 'static>>,
    on_disconnect: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_refresh: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_quick_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
}

impl Sidebar {
    pub fn new(
        connections: Vec<ConnectionConfig>,
        active_connection_id: Option<String>,
        active_tables: Vec<TableInfo>,
        table_filter: &Entity<InputState>,
    ) -> Self {
        Self {
            connections,
            active_connection_id,
            active_tables,
            table_filter: table_filter.clone(),
            selected_table: None,
            on_new_connection: None,
            on_select_connection: None,
            on_select_table: None,
            on_disconnect: None,
            on_refresh: None,
            on_quick_query: None,
        }
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
        F: Fn(TableInfo, &mut Window, &mut App) + 'static,
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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
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
                .px_2()
                .py_1()
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
                        .min_w_0()
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
                                .size(px(13.0))
                                .text_color(if is_active {
                                    ThemeColors::PRIMARY_BORDER
                                } else {
                                    ThemeColors::TEXT_MUTED
                                }),
                        )
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
                    Icon::new(IconName::ChevronDown)
                        .size(px(12.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .on_click(move |_, window, cx| {
                    if let Some(ref handler) = on_select {
                        handler(conn_id.clone(), window, cx);
                    }
                });

            conn_list = conn_list.child(conn_item);
        }

        if self.connections.is_empty() {
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
        }

        let filter = self.table_filter.read(cx).value().to_string();
        let filter = filter.trim().to_lowercase();
        let filtered_tables: Vec<TableInfo> = if filter.is_empty() {
            self.active_tables.clone()
        } else {
            self.active_tables
                .iter()
                .filter(|t| {
                    t.name.to_lowercase().contains(&filter)
                        || t.schema
                            .as_deref()
                            .is_some_and(|s| s.to_lowercase().contains(&filter))
                })
                .cloned()
                .collect()
        };

        let active_family = self
            .connections
            .iter()
            .find(|c| Some(c.id.as_str()) == self.active_connection_id.as_deref())
            .map(|c| c.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);
        let active_database = self
            .connections
            .iter()
            .find(|c| Some(c.id.as_str()) == self.active_connection_id.as_deref())
            .map(|c| c.database.clone())
            .unwrap_or_default();

        let on_quick = self.on_quick_query.clone();

        let mut grouped: BTreeMap<String, (Vec<TableInfo>, Vec<TableInfo>)> = BTreeMap::new();
        for tbl in filtered_tables {
            let schema = tbl
                .schema
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "default".to_string());
            let entry = grouped.entry(schema).or_default();
            if tbl.is_view() {
                entry.1.push(tbl);
            } else {
                entry.0.push(tbl);
            }
        }

        let search_row = h_flex()
            .w_full()
            .px_2()
            .py_1()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .child(Input::new(&self.table_filter).small().w_full());

        let database_row = h_flex()
            .w_full()
            .px_2()
            .py_1()
            .items_center()
            .gap_2()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .child(
                Icon::new(IconName::Database)
                    .size(px(13.0))
                    .text_color(ThemeColors::WARNING),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(if active_database.is_empty() {
                        family_label(active_family).to_string()
                    } else {
                        active_database
                    }),
            );

        let tables_pane = v_flex()
            .flex_1()
            .min_h_0()
            .child(database_row)
            .child(search_row)
            .child({
                let mut tbl_list = v_flex()
                    .id("sidebar_tables_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .gap_0p5()
                    .p_1();

                for (schema, (tables, views)) in grouped {
                    tbl_list = tbl_list.child(
                        h_flex()
                            .w_full()
                            .px_2()
                            .py_1()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(IconName::Folder)
                                    .size(px(13.0))
                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(schema.clone()),
                            ),
                    );

                    tbl_list = tbl_list.child(group_header("TABLES", tables.len(), IconName::Table));
                    for tbl in &tables {
                        tbl_list = tbl_list.child(table_row(
                            tbl,
                            &self.selected_table,
                            active_family,
                            self.on_select_table.clone(),
                            on_quick.clone(),
                        ));
                    }

                    tbl_list = tbl_list.child(group_header("VIEWS", views.len(), IconName::Eye));
                    if views.is_empty() {
                        tbl_list = tbl_list.child(empty_group_hint());
                    }
                    for tbl in &views {
                        tbl_list = tbl_list.child(table_row(
                            tbl,
                            &self.selected_table,
                            active_family,
                            self.on_select_table.clone(),
                            on_quick.clone(),
                        ));
                    }
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

fn family_label(family: DatabaseFamily) -> &'static str {
    match family {
        DatabaseFamily::Sqlite => "sqlite",
        DatabaseFamily::Postgres => "postgres",
        DatabaseFamily::MySql => "mysql",
    }
}

fn group_header(label: &'static str, count: usize, icon: IconName) -> impl IntoElement {
    h_flex()
        .w_full()
        .px_2()
        .py_1()
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .items_center()
                .gap_1p5()
                .child(
                    Icon::new(icon)
                        .size(px(12.0))
                        .text_color(ThemeColors::SUCCESS),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(label),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(ThemeColors::TEXT_FAINT)
                .child(count.to_string()),
        )
}

fn empty_group_hint() -> impl IntoElement {
    div()
        .px_6()
        .py_1()
        .text_xs()
        .text_color(ThemeColors::TEXT_FAINT)
        .child("No objects found")
}

fn table_row(
    tbl: &TableInfo,
    selected_table: &Option<String>,
    family: DatabaseFamily,
    on_select: Option<Rc<dyn Fn(TableInfo, &mut Window, &mut App) + 'static>>,
    on_quick: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let is_selected = selected_table.as_deref() == Some(&tbl.name);
    let info = tbl.clone();
    let info_click = info.clone();
    let qualified = tbl.qualified_name(family);
    let qualified_count = qualified.clone();
    let on_quick_select = on_quick.clone();
    let on_quick_count = on_quick.clone();
    let icon_name = if tbl.is_view() {
        IconName::Eye
    } else {
        IconName::Table
    };
    let row_id = format!(
        "tbl_row_{}_{}",
        tbl.schema.as_deref().unwrap_or("default"),
        tbl.name
    );

    h_flex()
        .id(ElementId::Name(row_id.into()))
        .w_full()
        .py_1()
        .px_2()
        .pl_6()
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
                            ThemeColors::SUCCESS
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
                    Button::new(ElementId::Name(format!("quick_sel_{}", info.name).into()))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Play)
                        .tooltip("SELECT * LIMIT 100")
                        .when_some(on_quick_select, |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(
                                    format!("SELECT * FROM {qualified} LIMIT 100;"),
                                    window,
                                    cx,
                                );
                            })
                        }),
                )
                .child(
                    Button::new(ElementId::Name(format!("quick_cnt_{}", info.name).into()))
                        .ghost()
                        .xsmall()
                        .tooltip("COUNT(*)")
                        .child("#")
                        .when_some(on_quick_count, |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(
                                    format!("SELECT COUNT(*) AS total_count FROM {qualified_count};"),
                                    window,
                                    cx,
                                );
                            })
                        }),
                ),
        )
        .on_click(move |_, window, cx| {
            if let Some(ref handler) = on_select {
                handler(info_click.clone(), window, cx);
            }
        })
}
